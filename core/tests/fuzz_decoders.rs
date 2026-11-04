//! PRD 14.3 PV-SEC-03: decoders never panic and rejected input never mutates state.
//! Decoders present in core: SyncEnvelope (DtnEngine::ingest), RevocationSnapshot
//! (DtnEngine::ingest_snapshot / verify_snapshot), TxPropose (ReceiverEngine::on_propose),
//! AckReceipt (PayerEngine::on_ack), ChequeSpendBody (derive_tx_id and the receiver).
//! Not in core: RedeemRequest, RecoveryCert (server-side only).
//! Case count: env PROPTEST_CASES (default 64).
mod support;

use meshpay_core::dtn_buffer::DtnEngine;
use meshpay_core::dtn_ingest_outcome::IngestOutcome;
use meshpay_core::dtn_snapshot::{snapshot_signing_message, verify_snapshot};
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_hashing::{derive_tx_id, spend_signing_message};
use meshpay_core::tx_keys::{compressed_public_key, sign, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend::ChequeSpend;
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::revocation_snapshot::RevocationSnapshot;
use meshpay_core::tx_wire::sync_envelope::SyncEnvelope;
use meshpay_core::tx_wire::tx_propose::TxPropose;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use prost::Message;
use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

const NOW: u64 = 1_900_000_000;
const DB_KEY: [u8; 32] = [5; 32];
const SNAPSHOT_SEED: [u8; 32] = [11; 32];
const PAYER_SEED: [u8; 32] = [7; 32];
const PAYEE_SEED: [u8; 32] = [9; 32];
const ACCOUNT: &[u8] = b"payee-account";

fn runner() -> TestRunner {
  let cases = std::env::var("PROPTEST_CASES")
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(64);
  TestRunner::new(Config {
    cases,
    failure_persistence: None,
    ..Config::default()
  })
}

fn public_key(seed: &[u8; 32]) -> Vec<u8> {
  compressed_public_key(&signing_key_from_seed(seed).unwrap())
}

fn count(path: &Path, sql: &str) -> i64 {
  let connection = Connection::open(path).unwrap();
  let hex_key: String = DB_KEY.iter().map(|b| format!("{b:02x}")).collect();
  connection
    .pragma_update(None, "key", format!("x'{hex_key}'"))
    .unwrap();
  connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Arbitrary bytes, or a golden vector with one bit flipped, one byte replaced, or truncated.
fn input(golden: Vec<u8>) -> impl Strategy<Value = Vec<u8>> {
  let length = golden.len();
  prop_oneof![
    prop::collection::vec(any::<u8>(), 0..8192),
    (0..length, 0u8..8).prop_map({
      let golden = golden.clone();
      move |(index, bit)| {
        let mut bytes = golden.clone();
        bytes[index] ^= 1 << bit;
        bytes
      }
    }),
    (0..length, any::<u8>()).prop_map({
      let golden = golden.clone();
      move |(index, byte)| {
        let mut bytes = golden.clone();
        bytes[index] = byte;
        bytes
      }
    }),
    (0..length).prop_map(move |cut| golden[..cut].to_vec()),
  ]
}

fn dtn_engine(directory: &Path) -> DtnEngine {
  DtnEngine::open_with_clock(
    &directory.join("dtn.db"),
    &DB_KEY,
    &public_key(&SNAPSHOT_SEED),
    5 * 1024 * 1024,
    1_000_000,
    Box::new(|| Duration::ZERO),
  )
  .unwrap()
}

fn signed_snapshot(snapshot: RevocationSnapshot) -> Vec<u8> {
  let key = signing_key_from_seed(&SNAPSHOT_SEED).unwrap();
  let signature = sign(&key, &snapshot_signing_message(&snapshot));
  RevocationSnapshot {
    server_signature: signature,
    ..snapshot
  }
  .encode_to_vec()
}

#[test]
fn sync_envelope_never_panics_and_only_stored_changes_the_buffer() {
  let directory = tempfile::tempdir().unwrap();
  let engine = dtn_engine(directory.path());
  let golden = SyncEnvelope::new(vec![1; 64], NOW, 3, 0).encode_to_vec();
  runner()
    .run(&input(golden), |bytes| {
      let (before_count, before_bytes) = (
        engine.envelope_count().unwrap(),
        engine.buffer_bytes().unwrap(),
      );
      match engine.ingest(&bytes, NOW) {
        Ok(IngestOutcome::Stored) => {
          prop_assert_eq!(engine.envelope_count().unwrap(), before_count + 1)
        }
        Ok(_) | Err(_) => {
          prop_assert_eq!(engine.envelope_count().unwrap(), before_count);
          prop_assert_eq!(engine.buffer_bytes().unwrap(), before_bytes);
        }
      }
      Ok(())
    })
    .unwrap();
}

#[test]
fn revocation_snapshot_never_panics_and_rejection_keeps_the_stored_snapshot() {
  let directory = tempfile::tempdir().unwrap();
  let engine = dtn_engine(directory.path());
  let key = signing_key_from_seed(&SNAPSHOT_SEED).unwrap();
  let golden = signed_snapshot(RevocationSnapshot {
    version: 1,
    issued_at_unix: NOW,
    revoked_signer_key_ids: vec![vec![1; 8]],
    ..Default::default()
  });
  // Validly signed snapshots with arbitrary fields exercise the code behind the signature check.
  let signed = (
    any::<u64>(),
    any::<u64>(),
    prop::collection::vec(prop::collection::vec(any::<u8>(), 0..40), 0..6),
    any::<bool>(),
  )
    .prop_map(|(version, issued_at_unix, ids, frozen)| {
      signed_snapshot(RevocationSnapshot {
        version,
        issued_at_unix,
        revoked_signer_key_ids: ids.clone(),
        new_proofs: ids,
        issuance_frozen: frozen,
        ..Default::default()
      })
    });
  runner()
    .run(&prop_oneof![input(golden), signed], |bytes| {
      let before = engine.snapshot_bytes().unwrap();
      let _ = verify_snapshot(&bytes, key.verifying_key());
      if engine.ingest_snapshot(&bytes).is_err() {
        prop_assert_eq!(engine.snapshot_bytes().unwrap(), before);
      }
      Ok(())
    })
    .unwrap();
}

struct Pair {
  directory: tempfile::TempDir,
  payer: PayerEngine,
  receiver: ReceiverEngine,
  propose: Vec<u8>,
  ack: Vec<u8>,
}

fn pair() -> Pair {
  let directory = tempfile::tempdir().unwrap();
  let payer = PayerEngine::open(
    &directory.path().join("payer.db"),
    &DB_KEY,
    &PAYER_SEED,
    &public_key(&PAYEE_SEED),
  )
  .unwrap();
  let receiver = ReceiverEngine::open(
    &directory.path().join("receiver.db"),
    &DB_KEY,
    &PAYEE_SEED,
    &public_key(&PAYER_SEED),
    ACCOUNT,
    support::issuer_trust(),
  )
  .unwrap();
  let body = ChequeSpendBody {
    cheque_id: b"cheque-00000001".to_vec(),
    payee_account_id: ACCOUNT.to_vec(),
    amount_units: 100,
    counter: 1,
    signed_at_unix: NOW,
    ..Default::default()
  };
  let propose = payer
    .propose(
      &body.encode_to_vec(),
      1,
      ACCOUNT,
      &support::stub_for(&body.encode_to_vec()),
    )
    .unwrap();
  let ack = receiver.on_propose(&propose, &support::context()).unwrap();
  Pair {
    directory,
    payer,
    receiver,
    propose,
    ack,
  }
}

#[test]
fn tx_propose_never_panics_and_rejection_stores_nothing() {
  let pair = pair();
  let path = pair.directory.path().join("receiver.db");
  let sql = "SELECT count(*) FROM incoming_cheques";
  let payer_key = signing_key_from_seed(&PAYER_SEED).unwrap();
  // Correctly signed propose around an arbitrary body: reaches tx_id derivation and body decode.
  let signed = prop::collection::vec(any::<u8>(), 0..300).prop_map(move |body| {
    let payer_signature = sign(&payer_key, &spend_signing_message(&body));
    TxPropose {
      stub: Some(support::stub_message_for(&body)),
      spend: Some(ChequeSpend {
        body,
        payer_signature,
      }),
    }
    .encode_to_vec()
  });
  runner()
    .run(&prop_oneof![input(pair.propose.clone()), signed], |bytes| {
      let before = count(&path, sql);
      let _ = derive_tx_id(1, &bytes);
      match pair.receiver.on_propose(&bytes, &support::context()) {
        Ok(_) => prop_assert!(count(&path, sql) <= before + 1),
        Err(_) => prop_assert_eq!(count(&path, sql), before),
      }
      Ok(())
    })
    .unwrap();
}

#[test]
fn ack_receipt_never_panics_and_rejection_leaves_the_cheque_locked() {
  let pair = pair();
  let path = pair.directory.path().join("payer.db");
  let sql = "SELECT count(*) FROM outgoing_cheques WHERE state = 'LOCKED_OUTGOING'";
  runner()
    .run(&input(pair.ack.clone()), |bytes| {
      let before = count(&path, sql);
      if pair.payer.on_ack(&bytes).is_err() {
        prop_assert_eq!(count(&path, sql), before);
      }
      Ok(())
    })
    .unwrap();
}
