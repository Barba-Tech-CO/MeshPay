use crate::prepared_propose::PreparedPropose;
use crate::spend_plan::SpendPlan;
use crate::tx_database::{open_database, sql_error, to_sql_integer};
use crate::tx_hashing::{ack_signing_message, spend_hash, spend_signing_message};
use crate::tx_keys::{
  sign, signing_key_from_seed, verify, verify_normalized, verifying_key_from_sec1,
};
use crate::tx_wire::ack_receipt::AckReceipt;
use crate::tx_wire::cheque_spend::ChequeSpend;
use crate::tx_wire::cheque_spend_body::ChequeSpendBody;
use crate::tx_wire::tx_propose::TxPropose;
use p256::ecdsa::{SigningKey, VerifyingKey};
use prost::Message;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

fn schema() -> &'static str {
  "
CREATE TABLE IF NOT EXISTS outgoing_cheques (
  tx_id BLOB PRIMARY KEY CHECK (length(tx_id) = 32),
  counter INTEGER NOT NULL UNIQUE,
  cheque_id BLOB NOT NULL UNIQUE,
  payee_account_id BLOB NOT NULL,
  amount_units INTEGER NOT NULL,
  signed_at_unix INTEGER NOT NULL,
  spend_hash BLOB NOT NULL CHECK (length(spend_hash) = 32),
  propose_bytes BLOB NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('LOCKED_OUTGOING', 'COMMITTED', 'EXPIRED')),
  ack_bytes BLOB,
  CHECK ((state = 'COMMITTED') = (ack_bytes IS NOT NULL))
) STRICT;"
}

/// Largest `TxPropose` accepted or produced; a lab spend is far smaller.
pub const MAX_PROPOSE_BYTES: usize = 4096;

/// Payer side of the two-phase commit. Software mode (LAB) signs with a seed held here;
/// external mode (Android Keystore, key not exportable) only knows the public key and takes
/// the signature from the caller through `prepare_propose` / `finish_propose`.
pub struct PayerEngine {
  connection: Mutex<Connection>,
  signing_key: Option<SigningKey>,
  own_key: VerifyingKey,
  payee_key: VerifyingKey,
}

impl PayerEngine {
  pub fn open(
    db_path: &Path,
    db_key: &[u8],
    signing_seed: &[u8],
    payee_public_key: &[u8],
  ) -> Result<Self, String> {
    let signing_key = signing_key_from_seed(signing_seed)?;
    Ok(Self {
      connection: Mutex::new(open_database(db_path, db_key, schema())?),
      own_key: *signing_key.verifying_key(),
      signing_key: Some(signing_key),
      payee_key: verifying_key_from_sec1(payee_public_key)?,
    })
  }

  /// External mode: no seed, `own_public_key` is the P-256 key (SEC1, 65 or 33 bytes) whose
  /// private half lives in the Keystore.
  pub fn open_external(
    db_path: &Path,
    db_key: &[u8],
    own_public_key: &[u8],
    payee_public_key: &[u8],
  ) -> Result<Self, String> {
    Ok(Self {
      connection: Mutex::new(open_database(db_path, db_key, schema())?),
      signing_key: None,
      own_key: verifying_key_from_sec1(own_public_key)?,
      payee_key: verifying_key_from_sec1(payee_public_key)?,
    })
  }

  /// Phase 1 (software mode). `spend_body` is a `ChequeSpendBody` with empty (or correct) `tx_id`.
  /// Intent and signed bytes are durable (WAL, synchronous=FULL) before this returns the
  /// `TxPropose` bytes. Same counter and content returns the stored bytes; other content errors.
  pub fn propose(
    &self,
    spend_body: &[u8],
    counter: u64,
    recipient_id: &[u8],
  ) -> Result<Vec<u8>, String> {
    let signing_key = self
      .signing_key
      .as_ref()
      .ok_or("external signer: use prepare_propose and finish_propose")?;
    let plan = SpendPlan::new(spend_body, counter, recipient_id)?;
    let signature = sign(signing_key, &spend_signing_message(&plan.body_bytes));
    self.persist(&plan, counter, signature)
  }

  /// External phase 1: validates, derives tx_id and returns the message to sign. Read-only.
  pub fn prepare_propose(
    &self,
    spend_body: &[u8],
    counter: u64,
    recipient_id: &[u8],
  ) -> Result<PreparedPropose, String> {
    let plan = SpendPlan::new(spend_body, counter, recipient_id)?;
    let existing_propose = Self::existing(
      &*self.connection()?,
      to_sql_integer(counter)?,
      &plan.body.cheque_id,
      &plan.tx_id,
    )?;
    Ok(PreparedPropose {
      message: spend_signing_message(&plan.body_bytes),
      body: plan.body_bytes,
      counter,
      existing_propose,
    })
  }

  /// External phase 2: `signature` is raw 64 bytes or ASN.1 DER (Android `SHA256withECDSA`).
  /// It is verified against the configured public key before anything is written; the row lands
  /// in one IMMEDIATE transaction. Same counter and content returns the stored bytes.
  pub fn finish_propose(
    &self,
    prepared: &PreparedPropose,
    signature: &[u8],
  ) -> Result<Vec<u8>, String> {
    let body =
      ChequeSpendBody::decode(prepared.body.as_slice()).map_err(|e| format!("spend body: {e}"))?;
    let plan = SpendPlan::new(&prepared.body, prepared.counter, &body.payee_account_id)?;
    let raw = verify_normalized(
      &self.own_key,
      &spend_signing_message(&plan.body_bytes),
      signature,
    )?;
    self.persist(&plan, prepared.counter, raw)
  }

  /// Stored propose bytes for this exact content; error when the counter or cheque is taken by
  /// different content.
  fn existing(
    connection: &Connection,
    counter_sql: i64,
    cheque_id: &[u8],
    tx_id: &[u8],
  ) -> Result<Option<Vec<u8>>, String> {
    let existing: Option<(Vec<u8>, Vec<u8>)> = connection
      .query_row(
        "SELECT tx_id, propose_bytes FROM outgoing_cheques WHERE counter = ?1 OR cheque_id = ?2",
        params![counter_sql, cheque_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()
      .map_err(sql_error)?;
    match existing {
      None => Ok(None),
      Some((stored_tx_id, stored_bytes)) if stored_tx_id == tx_id => Ok(Some(stored_bytes)),
      Some(_) => Err("counter or cheque already used with different content".to_owned()),
    }
  }

  fn persist(
    &self,
    plan: &SpendPlan,
    counter: u64,
    payer_signature: Vec<u8>,
  ) -> Result<Vec<u8>, String> {
    let counter_sql = to_sql_integer(counter)?;
    let amount_sql = to_sql_integer(plan.body.amount_units)?;
    let signed_at_sql = to_sql_integer(plan.body.signed_at_unix)?;

    let mut connection = self.connection()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(sql_error)?;
    if let Some(stored) =
      Self::existing(&transaction, counter_sql, &plan.body.cheque_id, &plan.tx_id)?
    {
      return Ok(stored);
    }

    let spend = ChequeSpend {
      payer_signature,
      body: plan.body_bytes.clone(),
    };
    let propose_bytes = TxPropose { spend: Some(spend) }.encode_to_vec();
    if propose_bytes.len() > MAX_PROPOSE_BYTES {
      return Err("propose exceeds the size cap".to_owned());
    }
    transaction
      .execute(
        "INSERT INTO outgoing_cheques
           (tx_id, counter, cheque_id, payee_account_id, amount_units, signed_at_unix, spend_hash, propose_bytes, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'LOCKED_OUTGOING')",
        params![
          plan.tx_id,
          counter_sql,
          plan.body.cheque_id,
          plan.body.payee_account_id,
          amount_sql,
          signed_at_sql,
          spend_hash(&plan.body_bytes),
          propose_bytes
        ],
      )
      .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(propose_bytes)
  }

  /// Phase 2: verified `AckReceipt` moves LOCKED_OUTGOING to COMMITTED. Idempotent.
  pub fn on_ack(&self, ack_bytes: &[u8]) -> Result<(), String> {
    let ack = AckReceipt::decode(ack_bytes).map_err(|e| format!("ack: {e}"))?;
    verify(
      &self.payee_key,
      &ack_signing_message(&ack.tx_id, &ack.spend_hash),
      &ack.payee_signature,
    )?;

    let mut connection = self.connection()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(sql_error)?;
    let (stored_hash, state): (Vec<u8>, String) = transaction
      .query_row(
        "SELECT spend_hash, state FROM outgoing_cheques WHERE tx_id = ?1",
        params![ack.tx_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()
      .map_err(sql_error)?
      .ok_or("ack for unknown tx_id")?;
    if stored_hash != ack.spend_hash {
      return Err("ack spend_hash differs from the stored spend".to_owned());
    }
    match state.as_str() {
      "COMMITTED" => return Ok(()),
      "LOCKED_OUTGOING" => {}
      other => return Err(format!("cannot commit a {other} cheque")),
    }
    transaction
      .execute(
        "UPDATE outgoing_cheques SET state = 'COMMITTED', ack_bytes = ?2 WHERE tx_id = ?1",
        params![ack.tx_id, ack_bytes],
      )
      .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
  }

  /// `TxPropose` bytes of every cheque still LOCKED_OUTGOING, byte for byte as first sent.
  pub fn resume_pending(&self) -> Result<Vec<Vec<u8>>, String> {
    let connection = self.connection()?;
    let mut statement = connection
      .prepare(
        "SELECT propose_bytes FROM outgoing_cheques WHERE state = 'LOCKED_OUTGOING' ORDER BY counter",
      )
      .map_err(sql_error)?;
    statement
      .query_map([], |row| row.get(0))
      .map_err(sql_error)?
      .collect::<Result<_, _>>()
      .map_err(sql_error)
  }

  /// LOCKED_OUTGOING cheques signed at least `expiry_secs` before `now_unix` become EXPIRED
  /// (never earlier: until then they stay redeliverable). Returns how many expired.
  pub fn expire_locked(&self, now_unix: u64, expiry_secs: u64) -> Result<usize, String> {
    let cutoff = to_sql_integer(now_unix.saturating_sub(expiry_secs).min(i64::MAX as u64))?;
    self
      .connection()?
      .execute(
        "UPDATE outgoing_cheques SET state = 'EXPIRED'
         WHERE state = 'LOCKED_OUTGOING' AND signed_at_unix <= ?1",
        params![cutoff],
      )
      .map_err(sql_error)
  }

  fn connection(&self) -> Result<MutexGuard<'_, Connection>, String> {
    self
      .connection
      .lock()
      .map_err(|_| "database lock poisoned".to_owned())
  }
}
