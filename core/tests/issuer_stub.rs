//! PRD 7.4, PV-SEC-05: the receiver only trusts a stub sealed by an ISSUER key whose
//! `ServerKeyCert` chains to the pinned root and is current at the protocol clock.
mod support;

use meshpay_core::api::{lab_cheque_stub, lab_issuer_cert, lab_issuer_root_public_key};
use meshpay_core::issuer_trust::IssuerTrust;
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::cheque_stub::ChequeStub;
use meshpay_core::tx_wire::cheque_stub_body::ChequeStubBody;
use prost::Message;
use support::{CERT_NOT_AFTER, E, T_SRV};

const PAYER_SEED: [u8; 32] = [7; 32];
const PAYEE_SEED: [u8; 32] = [9; 32];
const DB_KEY: [u8; 32] = [3; 32];
const OWN_ACCOUNT: &[u8] = b"payee-account";
const CHEQUE_ID: &[u8] = b"cheque-00000001";

fn public(seed: &[u8; 32]) -> Vec<u8> {
  compressed_public_key(&signing_key_from_seed(seed).unwrap())
}

/// Receiver pinned to `issuer`; stores one propose carrying `stub` and returns the outcome.
fn store_with(issuer: IssuerTrust, stub: &[u8], at_unix: u64) -> Result<Vec<u8>, String> {
  let directory = tempfile::tempdir().unwrap();
  let payer = PayerEngine::open(
    &directory.path().join("p.db"),
    &DB_KEY,
    &PAYER_SEED,
    &public(&PAYEE_SEED),
  )
  .unwrap();
  let receiver = ReceiverEngine::open(
    &directory.path().join("r.db"),
    &DB_KEY,
    &PAYEE_SEED,
    &public(&PAYER_SEED),
    OWN_ACCOUNT,
    issuer,
  )
  .unwrap();
  let body = ChequeSpendBody {
    cheque_id: CHEQUE_ID.to_vec(),
    payee_account_id: OWN_ACCOUNT.to_vec(),
    amount_units: 100,
    counter: 1,
    ..Default::default()
  }
  .encode_to_vec();
  let propose = payer.propose(&body, 1, OWN_ACCOUNT, stub).unwrap();
  let mut context = support::context();
  context.now.unix = at_unix;
  context.snapshot_issued_at_unix = Some(at_unix);
  context.now.t_srv_unix = at_unix;
  receiver.store_propose(&propose, &context)
}

fn store(stub: &[u8]) -> Result<Vec<u8>, String> {
  store_with(support::issuer_trust(), stub, T_SRV)
}

#[test]
fn sealed_stub_is_accepted() {
  assert!(store(&support::stub_expiring(CHEQUE_ID, T_SRV + E)).is_ok());
}

#[test]
fn tampered_expiry_is_refused() {
  let mut stub =
    ChequeStub::decode(support::stub_expiring(CHEQUE_ID, T_SRV + 3600).as_slice()).unwrap();
  let mut body = ChequeStubBody::decode(stub.body.as_slice()).unwrap();
  body.expires_at_unix = T_SRV + E;
  stub.body = body.encode_to_vec();
  assert_eq!(
    store(&stub.encode_to_vec()),
    Err("stub server signature invalid".to_owned())
  );
}

#[test]
fn unsigned_stub_is_refused() {
  let stub = ChequeStub {
    body: support::stub_body(CHEQUE_ID, T_SRV + E).encode_to_vec(),
    server_signature: Vec::new(),
  };
  assert_eq!(
    store(&stub.encode_to_vec()),
    Err("stub server signature invalid".to_owned())
  );
}

#[test]
fn stub_for_another_payer_key_is_refused() {
  let body = ChequeStubBody {
    signer_pubkey: public(&[11; 32]),
    ..support::stub_body(CHEQUE_ID, T_SRV + E)
  };
  assert_eq!(
    store(&support::seal(&body)),
    Err("stub issued to a different payer key".to_owned())
  );
}

#[test]
fn amount_above_stub_maximum_is_refused() {
  let body = ChequeStubBody {
    max_amount_units: 99,
    ..support::stub_body(CHEQUE_ID, T_SRV + E)
  };
  assert_eq!(
    store(&support::seal(&body)),
    Err("amount exceeds the stub maximum".to_owned())
  );
}

#[test]
fn stub_past_cert_lifetime_is_refused() {
  assert_eq!(
    store(&support::stub_expiring(CHEQUE_ID, CERT_NOT_AFTER + 1)),
    Err("stub expires after its server key cert".to_owned())
  );
}

#[test]
fn expired_cert_is_refused_at_protocol_now() {
  assert_eq!(
    store_with(
      support::issuer_trust(),
      &support::stub_expiring(CHEQUE_ID, CERT_NOT_AFTER),
      CERT_NOT_AFTER + 1,
    ),
    Err("server issuer key cert not current".to_owned())
  );
}

#[test]
fn bad_certs_are_refused() {
  let root = support::issuer_root_public_key();
  let (before, after) = (T_SRV, T_SRV + 86_400);
  for (cert, reason) in [
    (
      support::cert_with([9; 32], [8; 32], 2, before, after),
      "server key cert: wrong role",
    ),
    (
      support::cert_with([4; 32], [8; 32], 1, before, after),
      "server key cert: bad root signature",
    ),
    (
      support::cert_with([9; 32], [8; 32], 1, before, before + 31 * 86_400),
      "server key cert: lifetime outside 1s..=30 days",
    ),
  ] {
    assert_eq!(
      IssuerTrust::new(&root, &cert).err(),
      Some(reason.to_owned())
    );
  }
}

#[test]
fn stub_from_another_issuer_key_is_refused() {
  let other = support::cert_with([9; 32], [12; 32], 1, T_SRV - 10, T_SRV + 86_400);
  let issuer = IssuerTrust::new(&support::issuer_root_public_key(), &other).unwrap();
  assert_eq!(
    store_with(issuer, &support::stub_expiring(CHEQUE_ID, T_SRV + E), T_SRV),
    Err("stub server signature invalid".to_owned())
  );
}

#[test]
fn lab_stub_verifies_only_under_the_lab_root() {
  let stub = lab_cheque_stub(CHEQUE_ID.to_vec(), public(&PAYER_SEED), 100, T_SRV + E).unwrap();
  let lab = IssuerTrust::new(&lab_issuer_root_public_key(), &lab_issuer_cert(T_SRV - 60)).unwrap();
  assert!(store_with(lab, &stub, T_SRV).is_ok());
  assert_eq!(
    store(&stub),
    Err("stub server signature invalid".to_owned())
  );
  assert!(IssuerTrust::new(&support::issuer_root_public_key(), &lab_issuer_cert(T_SRV)).is_err());
}
