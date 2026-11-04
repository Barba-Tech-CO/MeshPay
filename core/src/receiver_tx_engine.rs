use crate::acceptance_context::AcceptanceContext;
use crate::cheque_acceptance::ChequeAcceptance;
use crate::issuer_trust::IssuerTrust;
use crate::payer_tx_engine::MAX_PROPOSE_BYTES;
use crate::prepared_ack::PreparedAck;
use crate::receiver_clock_row::ReceiverClockRow;
use crate::tx_database::{open_database, sql_error, to_sql_integer};
use crate::tx_hashing::{ack_signing_message, derive_tx_id, spend_hash, spend_signing_message};
use crate::tx_keys::{
  compressed_public_key_of, sign, signing_key_from_seed, verify, verify_normalized,
  verifying_key_from_sec1,
};
use crate::tx_wire::ack_receipt::AckReceipt;
use crate::tx_wire::cheque_spend_body::ChequeSpendBody;
use crate::tx_wire::tx_propose::TxPropose;
use p256::ecdsa::{SigningKey, VerifyingKey};
use prost::Message;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

fn schema() -> &'static str {
  "
CREATE TABLE IF NOT EXISTS incoming_cheques (
  tx_id BLOB PRIMARY KEY CHECK (length(tx_id) = 32),
  cheque_id BLOB NOT NULL UNIQUE,
  counter INTEGER NOT NULL,
  payee_account_id BLOB NOT NULL,
  amount_units INTEGER NOT NULL,
  spend_hash BLOB NOT NULL CHECK (length(spend_hash) = 32),
  propose_bytes BLOB NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('RECEIVED', 'ACKED')),
  ack_bytes BLOB,
  CHECK ((state = 'ACKED') = (ack_bytes IS NOT NULL))
) STRICT;
CREATE TABLE IF NOT EXISTS receiver_clock (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  t_srv_unix INTEGER NOT NULL,
  anchor_unix INTEGER NOT NULL,
  monotonic_nanos INTEGER NOT NULL,
  boot_count INTEGER,
  max_now_unix INTEGER NOT NULL,
  rebooted INTEGER NOT NULL CHECK (rebooted IN (0, 1))
) STRICT;"
}

fn from_sql_integer(value: i64) -> Result<u64, String> {
  u64::try_from(value).map_err(|_| "database: negative integer".to_owned())
}

/// Receiver side of the two-phase commit. Software mode (LAB) signs the `AckReceipt` with a
/// seed held here; external mode (Keystore) only knows the public key.
pub struct ReceiverEngine {
  connection: Mutex<Connection>,
  signing_key: Option<SigningKey>,
  own_key: VerifyingKey,
  payer_key: VerifyingKey,
  own_account_id: Vec<u8>,
  issuer: IssuerTrust,
}

impl ReceiverEngine {
  pub fn open(
    db_path: &Path,
    db_key: &[u8],
    signing_seed: &[u8],
    payer_public_key: &[u8],
    own_account_id: &[u8],
    issuer: IssuerTrust,
  ) -> Result<Self, String> {
    let signing_key = signing_key_from_seed(signing_seed)?;
    let own_key = *signing_key.verifying_key();
    Self::build(
      db_path,
      db_key,
      Some(signing_key),
      own_key,
      payer_public_key,
      own_account_id,
      issuer,
    )
  }

  /// External mode: no seed; `own_public_key` is the Keystore P-256 key (SEC1, 65 or 33 bytes).
  pub fn open_external(
    db_path: &Path,
    db_key: &[u8],
    own_public_key: &[u8],
    payer_public_key: &[u8],
    own_account_id: &[u8],
    issuer: IssuerTrust,
  ) -> Result<Self, String> {
    Self::build(
      db_path,
      db_key,
      None,
      verifying_key_from_sec1(own_public_key)?,
      payer_public_key,
      own_account_id,
      issuer,
    )
  }

  fn build(
    db_path: &Path,
    db_key: &[u8],
    signing_key: Option<SigningKey>,
    own_key: VerifyingKey,
    payer_public_key: &[u8],
    own_account_id: &[u8],
    issuer: IssuerTrust,
  ) -> Result<Self, String> {
    if own_account_id.is_empty() {
      return Err("own account id must not be empty".to_owned());
    }
    Ok(Self {
      connection: Mutex::new(open_database(db_path, db_key, schema())?),
      signing_key,
      own_key,
      payer_key: verifying_key_from_sec1(payer_public_key)?,
      own_account_id: own_account_id.to_vec(),
      issuer,
    })
  }

  /// Verifies, durably stores and acknowledges. Same propose twice returns the same ack.
  pub fn on_propose(
    &self,
    propose_bytes: &[u8],
    context: &AcceptanceContext,
  ) -> Result<Vec<u8>, String> {
    if self.signing_key.is_none() {
      return Err("external signer: use store_propose, prepare_ack and finish_ack".to_owned());
    }
    let tx_id = self.store_propose(propose_bytes, context)?;
    self.issue_ack(&tx_id)
  }

  /// Step 1 of `on_propose`: row lands as RECEIVED. Returns the tx_id. A new cheque must pass
  /// `ChequeAcceptance::check` on the stub `expires_at_unix`; redelivery of a stored one does not
  /// re-check, so the same propose keeps returning the same tx_id.
  pub fn store_propose(
    &self,
    propose_bytes: &[u8],
    context: &AcceptanceContext,
  ) -> Result<Vec<u8>, String> {
    if propose_bytes.len() > MAX_PROPOSE_BYTES {
      return Err("propose exceeds the size cap".to_owned());
    }
    let propose = TxPropose::decode(propose_bytes).map_err(|e| format!("propose: {e}"))?;
    let spend = propose.spend.ok_or("propose without spend")?;
    let stub = propose.stub.ok_or("propose without stub")?;
    verify(
      &self.payer_key,
      &spend_signing_message(&spend.body),
      &spend.payer_signature,
    )?;
    let body = ChequeSpendBody::decode(spend.body.as_slice()).map_err(|e| format!("body: {e}"))?;
    if body.payee_account_id != self.own_account_id {
      return Err("cheque is not addressed to this account".to_owned());
    }
    let tx_id = derive_tx_id(body.counter, &spend.body)?;
    if body.tx_id != tx_id {
      return Err("tx_id does not match counter and content".to_owned());
    }
    // ponytail: revoked_server_key_ids of the snapshot not checked; the receiver keeps only the
    // snapshot's issued_at. Persist the revoked list with the clock row when the kill switch lands.
    let stub_body = self.issuer.verify_stub(&stub)?;
    if stub_body.cheque_id != body.cheque_id {
      return Err("stub names a different cheque".to_owned());
    }
    if stub_body.signer_pubkey != compressed_public_key_of(&self.payer_key) {
      return Err("stub issued to a different payer key".to_owned());
    }
    if body.amount_units > stub_body.max_amount_units {
      return Err("amount exceeds the stub maximum".to_owned());
    }
    let hash = spend_hash(&spend.body);
    let counter_sql = to_sql_integer(body.counter)?;
    let amount_sql = to_sql_integer(body.amount_units)?;

    let mut connection = self.connection()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(sql_error)?;
    let existing: Option<(Vec<u8>, Vec<u8>)> = transaction
      .query_row(
        "SELECT tx_id, spend_hash FROM incoming_cheques WHERE tx_id = ?1 OR cheque_id = ?2",
        params![tx_id, body.cheque_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()
      .map_err(sql_error)?;
    if let Some((stored_tx_id, stored_hash)) = existing {
      return if stored_tx_id == tx_id && stored_hash == hash {
        Ok(tx_id)
      } else {
        Err("tx_id or cheque already stored with different content".to_owned())
      };
    }
    self.issuer.check_current(context.now.unix)?;
    ChequeAcceptance::check(
      stub_body.expires_at_unix,
      context.now,
      context.snapshot_issued_at_unix,
      context.expiry_secs,
    )?;
    transaction
      .execute(
        "INSERT INTO incoming_cheques
           (tx_id, cheque_id, counter, payee_account_id, amount_units, spend_hash, propose_bytes, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'RECEIVED')",
        params![
          tx_id,
          body.cheque_id,
          counter_sql,
          body.payee_account_id,
          amount_sql,
          hash,
          propose_bytes
        ],
      )
      .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(tx_id)
  }

  /// Step 2 of `on_propose` (software mode): RECEIVED becomes ACKED with the signed `AckReceipt`.
  pub fn issue_ack(&self, tx_id: &[u8]) -> Result<Vec<u8>, String> {
    self.commit_ack(tx_id, None)
  }

  /// External step 2a: message to sign for a stored propose. Read-only.
  pub fn prepare_ack(&self, tx_id: &[u8]) -> Result<PreparedAck, String> {
    let (hash, existing_ack) = Self::load_row(&*self.connection()?, tx_id)?;
    Ok(PreparedAck {
      tx_id: tx_id.to_vec(),
      message: ack_signing_message(tx_id, &hash),
      existing_ack,
    })
  }

  /// External step 2b: `signature` (raw or DER) is verified against the own public key, then the
  /// ack is persisted. An already stored ack is returned unchanged.
  pub fn finish_ack(&self, tx_id: &[u8], signature: &[u8]) -> Result<Vec<u8>, String> {
    self.commit_ack(tx_id, Some(signature))
  }

  fn load_row(connection: &Connection, tx_id: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    connection
      .query_row(
        "SELECT spend_hash, ack_bytes FROM incoming_cheques WHERE tx_id = ?1",
        params![tx_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()
      .map_err(sql_error)?
      .ok_or_else(|| "unknown tx_id".to_owned())
  }

  fn commit_ack(&self, tx_id: &[u8], external: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut connection = self.connection()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(sql_error)?;
    let (hash, ack_bytes) = Self::load_row(&transaction, tx_id)?;
    if let Some(stored) = ack_bytes {
      return Ok(stored);
    }
    let message = ack_signing_message(tx_id, &hash);
    let payee_signature = match external {
      Some(signature) => verify_normalized(&self.own_key, &message, signature)?,
      None => sign(
        self
          .signing_key
          .as_ref()
          .ok_or("external signer: use prepare_ack and finish_ack")?,
        &message,
      ),
    };
    let ack = AckReceipt {
      payee_signature,
      tx_id: tx_id.to_vec(),
      spend_hash: hash,
    }
    .encode_to_vec();
    transaction
      .execute(
        "UPDATE incoming_cheques SET state = 'ACKED', ack_bytes = ?2 WHERE tx_id = ?1",
        params![tx_id, ack],
      )
      .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(ack)
  }

  /// Persisted protocol-clock state, `None` until the first verified snapshot.
  pub fn load_clock(&self) -> Result<Option<ReceiverClockRow>, String> {
    let row = self
      .connection()?
      .query_row(
        "SELECT t_srv_unix, anchor_unix, monotonic_nanos, boot_count, max_now_unix, rebooted FROM receiver_clock WHERE id = 1",
        [],
        |row| {
          Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, bool>(5)?,
          ))
        },
      )
      .optional()
      .map_err(sql_error)?;
    row
      .map(|(t_srv, anchor, monotonic, boot, max_now, rebooted)| {
        Ok(ReceiverClockRow {
          t_srv_unix: from_sql_integer(t_srv)?,
          anchor_unix: from_sql_integer(anchor)?,
          monotonic_nanos: from_sql_integer(monotonic)?,
          boot_count: boot.map(from_sql_integer).transpose()?,
          max_now_unix: from_sql_integer(max_now)?,
          rebooted,
        })
      })
      .transpose()
  }

  /// Conditional write: never replaces a row with a stale one (older `t_srv`, or same `t_srv`
  /// with a lower `max_now` or a cleared reboot flag), so two engines on one file cannot lose
  /// each other's updates.
  pub fn save_clock(&self, clock: &ReceiverClockRow) -> Result<(), String> {
    self
      .connection()?
      .execute(
        "INSERT INTO receiver_clock (id, t_srv_unix, anchor_unix, monotonic_nanos, boot_count, max_now_unix, rebooted) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (id) DO UPDATE SET t_srv_unix = excluded.t_srv_unix, anchor_unix = excluded.anchor_unix, monotonic_nanos = excluded.monotonic_nanos, boot_count = excluded.boot_count, max_now_unix = excluded.max_now_unix, rebooted = excluded.rebooted
         WHERE excluded.t_srv_unix > receiver_clock.t_srv_unix OR (excluded.t_srv_unix = receiver_clock.t_srv_unix AND excluded.max_now_unix >= receiver_clock.max_now_unix AND excluded.rebooted >= receiver_clock.rebooted)",
        params![
          to_sql_integer(clock.t_srv_unix)?,
          to_sql_integer(clock.anchor_unix)?,
          to_sql_integer(clock.monotonic_nanos)?,
          clock.boot_count.map(to_sql_integer).transpose()?,
          to_sql_integer(clock.max_now_unix)?,
          clock.rebooted,
        ],
      )
      .map_err(sql_error)?;
    Ok(())
  }

  fn connection(&self) -> Result<MutexGuard<'_, Connection>, String> {
    self
      .connection
      .lock()
      .map_err(|_| "database lock poisoned".to_owned())
  }
}
