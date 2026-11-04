mod support;

use meshpay_core::api::{DtnBuffer, ReceiverTxEngine};
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use prost::Message;
use std::time::{SystemTime, UNIX_EPOCH};
use support::{T_SRV, cert_with, issuer_cert, issuer_root_public_key, signed_snapshot};

const DB_KEY: [u8; 32] = [6; 32];
const PAYEE_SEED: [u8; 32] = [4; 32];
const OWN_ACCOUNT: &[u8] = b"receiver-account";
const SNAPSHOT_ROLE: i32 = 2;
const ISSUER_ROLE: i32 = 1;
const DAY: u64 = 86_400;
const BACKDATE: u64 = 29 * DAY;

fn snapshot_cert(role: i32, root_seed: [u8; 32], not_before: u64, not_after: u64) -> Vec<u8> {
  cert_with(root_seed, [5; 32], role, not_before, not_after)
}

#[test]
fn lab_snapshot_cert_accepts_old_and_current_snapshot_times() {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let old_timestamp = now - BACKDATE;
  let cert = meshpay_core::lab_issuer::snapshot_cert(old_timestamp);
  let root_public_key = meshpay_core::lab_issuer::root_public_key();

  assert!(
    meshpay_core::api::verify_snapshot_cert_at_time(
      root_public_key.clone(),
      cert.clone(),
      old_timestamp,
    )
    .is_ok()
  );
  assert!(meshpay_core::api::verify_snapshot_cert_at_time(root_public_key, cert, now).is_ok());
}

#[test]
fn receiver_rejects_issuer_role_snapshot_certificate() {
  let directory = tempfile::tempdir().unwrap();
  let result = ReceiverTxEngine::open(
    directory
      .path()
      .join("receiver.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap()),
    OWN_ACCOUNT.to_vec(),
    snapshot_cert(ISSUER_ROLE, [9; 32], T_SRV - DAY, T_SRV + DAY),
    DAY,
    issuer_root_public_key(),
    issuer_cert(),
  );
  assert!(result.is_err());
}

#[test]
fn receiver_rejects_snapshot_cert_signed_by_another_root() {
  let directory = tempfile::tempdir().unwrap();
  let result = ReceiverTxEngine::open(
    directory
      .path()
      .join("receiver.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap()),
    OWN_ACCOUNT.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [10; 32], T_SRV - DAY, T_SRV + DAY),
    DAY,
    issuer_root_public_key(),
    issuer_cert(),
  );
  assert!(result.is_err());
}

#[test]
fn receiver_accepts_good_chain_and_rejects_snapshot_outside_cert_window() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = ReceiverTxEngine::open(
    directory
      .path()
      .join("receiver.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap()),
    OWN_ACCOUNT.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV - 10, T_SRV + 10),
    DAY,
    issuer_root_public_key(),
    issuer_cert(),
  );
  assert!(receiver.is_ok());
  assert_eq!(
    receiver
      .unwrap()
      .install_snapshot(signed_snapshot(T_SRV), 0, Some(1)),
    Ok(T_SRV)
  );
  let outside = ReceiverTxEngine::open(
    directory
      .path()
      .join("outside.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap()),
    OWN_ACCOUNT.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV + 1, T_SRV + DAY),
    DAY,
    issuer_root_public_key(),
    issuer_cert(),
  );
  assert!(outside.is_ok());
  assert!(
    outside
      .unwrap()
      .install_snapshot(signed_snapshot(T_SRV), 0, Some(1))
      .is_err()
  );
  let expired = ReceiverTxEngine::open(
    directory
      .path()
      .join("expired.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap()),
    OWN_ACCOUNT.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV - DAY, T_SRV - 1),
    DAY,
    issuer_root_public_key(),
    issuer_cert(),
  )
  .unwrap();
  assert!(
    expired
      .install_snapshot(signed_snapshot(T_SRV), 0, Some(1))
      .is_err()
  );
}

#[test]
fn receiver_accepts_both_inclusive_cert_window_boundaries() {
  let directory = tempfile::tempdir().unwrap();
  for (name, issued_at) in [("not-before", T_SRV - 10), ("not-after", T_SRV + 10)] {
    let receiver = ReceiverTxEngine::open(
      directory
        .path()
        .join(format!("{name}.db"))
        .to_string_lossy()
        .into_owned(),
      DB_KEY.to_vec(),
      PAYEE_SEED.to_vec(),
      compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap()),
      OWN_ACCOUNT.to_vec(),
      snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV - 10, T_SRV + 10),
      DAY,
      issuer_root_public_key(),
      issuer_cert(),
    )
    .unwrap();

    assert_eq!(
      receiver.install_snapshot(signed_snapshot(issued_at), 0, Some(1)),
      Ok(issued_at)
    );
  }
}

#[test]
fn rejected_out_of_window_snapshot_leaves_cheques_refused_without_clock_anchor() {
  let directory = tempfile::tempdir().unwrap();
  let payer_public_key = compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap());
  let payee_public_key = compressed_public_key(&signing_key_from_seed(&PAYEE_SEED).unwrap());
  let payer = PayerEngine::open(
    &directory.path().join("payer.db"),
    &DB_KEY,
    &[7; 32],
    &payee_public_key,
  )
  .unwrap();
  let receiver = ReceiverTxEngine::open(
    directory
      .path()
      .join("receiver.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    payer_public_key,
    OWN_ACCOUNT.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV - 10, T_SRV + 10),
    DAY,
    issuer_root_public_key(),
    issuer_cert(),
  )
  .unwrap();
  let spend = ChequeSpendBody {
    cheque_id: b"window-check-cheque".to_vec(),
    payee_account_id: OWN_ACCOUNT.to_vec(),
    amount_units: 1,
    counter: 1,
    ..Default::default()
  }
  .encode_to_vec();
  let stub = support::stub_for(&spend);
  let propose = payer.propose(&spend, 1, OWN_ACCOUNT, &stub).unwrap();

  assert_eq!(
    receiver.install_snapshot(signed_snapshot(T_SRV + 11), 0, Some(1)),
    Err("snapshot issued_at outside server key cert window".to_owned())
  );
  assert_eq!(
    receiver.on_propose(propose, 0, Some(1)),
    Err("no revocation snapshot".to_owned())
  );
}

#[test]
fn dtn_buffer_requires_snapshot_cert_and_checks_issued_at_window() {
  let directory = tempfile::tempdir().unwrap();
  let valid_buffer = DtnBuffer::open(
    directory
      .path()
      .join("valid-dtn.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV - 10, T_SRV + 10),
    issuer_root_public_key(),
    100_000,
  )
  .unwrap();
  assert_eq!(valid_buffer.ingest_snapshot(signed_snapshot(T_SRV)), Ok(1));
  assert!(
    valid_buffer
      .ingest_snapshot(signed_snapshot(T_SRV + 1))
      .is_err()
  );
  let buffer = DtnBuffer::open(
    directory
      .path()
      .join("dtn.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    snapshot_cert(SNAPSHOT_ROLE, [9; 32], T_SRV + 1, T_SRV + DAY),
    issuer_root_public_key(),
    100_000,
  );
  assert!(buffer.is_ok());
  assert!(
    buffer
      .unwrap()
      .ingest_snapshot(signed_snapshot(T_SRV))
      .is_err()
  );
}
