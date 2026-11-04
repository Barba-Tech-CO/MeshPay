//! PRD 4.3: the receiver refuses a new cheque whose stub was issued by a server key revoked in
//! the latest accepted snapshot. The revoked list is persisted beside the clock row.
mod support;

use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::receiver_clock_row::ReceiverClockRow;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use prost::Message;
use support::{E, T_SRV};

const PAYER_SEED: [u8; 32] = [7; 32];
const PAYEE_SEED: [u8; 32] = [9; 32];
const DB_KEY: [u8; 32] = [3; 32];
const OWN_ACCOUNT: &[u8] = b"payee-account";
const REVOKED_MESSAGE: &str = "stub issued by a revoked server key";

fn public(seed: &[u8; 32]) -> Vec<u8> {
  compressed_public_key(&signing_key_from_seed(seed).unwrap())
}

fn clock_at(t_srv_unix: u64) -> ReceiverClockRow {
  ReceiverClockRow {
    t_srv_unix,
    anchor_unix: t_srv_unix,
    monotonic_nanos: 0,
    boot_count: None,
    max_now_unix: t_srv_unix,
    rebooted: false,
  }
}

fn propose(cheque_id: &[u8]) -> Vec<u8> {
  let directory = tempfile::tempdir().unwrap();
  let payer = PayerEngine::open(
    &directory.path().join("p.db"),
    &DB_KEY,
    &PAYER_SEED,
    &public(&PAYEE_SEED),
  )
  .unwrap();
  let body = ChequeSpendBody {
    cheque_id: cheque_id.to_vec(),
    payee_account_id: OWN_ACCOUNT.to_vec(),
    amount_units: 100,
    counter: 1,
    ..Default::default()
  }
  .encode_to_vec();
  payer
    .propose(
      &body,
      1,
      OWN_ACCOUNT,
      &support::stub_expiring(cheque_id, T_SRV + E),
    )
    .unwrap()
}

fn receiver(directory: &tempfile::TempDir) -> ReceiverEngine {
  ReceiverEngine::open(
    &directory.path().join("r.db"),
    &DB_KEY,
    &PAYEE_SEED,
    &public(&PAYER_SEED),
    OWN_ACCOUNT,
    support::issuer_trust(),
  )
  .unwrap()
}

#[test]
fn revoked_server_key_rejects_a_new_cheque() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV), &[support::issuer_key_id()])
    .unwrap();
  assert_eq!(
    receiver.store_propose(&propose(b"cheque-00000001"), &support::context()),
    Err(REVOKED_MESSAGE.to_owned())
  );
}

#[test]
fn redelivery_of_a_stored_cheque_still_returns_the_same_tx_id() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  let propose = propose(b"cheque-00000001");
  let tx_id = receiver
    .store_propose(&propose, &support::context())
    .unwrap();
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV + 10), &[support::issuer_key_id()])
    .unwrap();
  assert_eq!(
    receiver.store_propose(&propose, &support::context()),
    Ok(tx_id)
  );
}

#[test]
fn older_snapshot_does_not_unrevoke() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV), &[support::issuer_key_id()])
    .unwrap();
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV - 10), &[])
    .unwrap();
  assert_eq!(
    receiver.store_propose(&propose(b"cheque-00000001"), &support::context()),
    Err(REVOKED_MESSAGE.to_owned())
  );
}

#[test]
fn newer_snapshot_replaces_the_revoked_list() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV), &[support::issuer_key_id()])
    .unwrap();
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV + 10), &[])
    .unwrap();
  assert!(
    receiver
      .store_propose(&propose(b"cheque-00000001"), &support::context())
      .is_ok()
  );
}

#[test]
fn empty_revoked_list_accepts() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV), &[])
    .unwrap();
  assert!(
    receiver
      .store_propose(&propose(b"cheque-00000001"), &support::context())
      .is_ok()
  );
}

#[test]
fn same_issued_at_replaces_the_revoked_list() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV), &[])
    .unwrap();
  receiver
    .save_clock_with_revoked_keys(&clock_at(T_SRV), &[support::issuer_key_id()])
    .unwrap();
  assert_eq!(
    receiver.store_propose(&propose(b"cheque-00000001"), &support::context()),
    Err(REVOKED_MESSAGE.to_owned())
  );
}

#[test]
fn key_id_not_16_bytes_is_refused_and_nothing_is_written() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = receiver(&directory);
  assert_eq!(
    receiver.save_clock_with_revoked_keys(&clock_at(T_SRV), &[vec![1; 15]]),
    Err("revoked server key id must be 16 bytes".to_owned())
  );
  assert_eq!(receiver.load_clock(), Ok(None));
}
