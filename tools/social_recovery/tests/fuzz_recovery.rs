//! Fuzz recovery verification and cancellation boundaries. Run 1,000,000 cases with
//! `PROPTEST_CASES=1000000 cargo test --release --test fuzz_recovery`.
use p256::ecdsa::SigningKey;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use social_recovery::keys;
use social_recovery::ledger::Ledger;
use social_recovery::recovery_cancel::RecoveryCancel;
use social_recovery::recovery_cert::RecoveryCert;

const ACCOUNT: &str = "fuzz-account";

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

fn key(byte: u8) -> SigningKey {
  SigningKey::from_slice(&[byte; 32]).unwrap()
}

fn ledger(root: &SigningKey, payment: &SigningKey) -> Ledger {
  let mut ledger = Ledger::new(key(3));
  ledger.register(
    ACCOUNT,
    *root.verifying_key(),
    *payment.verifying_key(),
    1_000,
  );
  ledger
}

fn arbitrary_bytes() -> impl Strategy<Value = Vec<u8>> {
  prop::collection::vec(any::<u8>(), 0..4096)
}

fn candidate_cert(bytes: Vec<u8>, signature: Vec<u8>) -> RecoveryCert {
  let old_epoch = bytes
    .get(..8)
    .and_then(|value| value.try_into().ok())
    .map(u64::from_be_bytes)
    .unwrap_or_default();
  let new_epoch = bytes
    .get(8..16)
    .and_then(|value| value.try_into().ok())
    .map(u64::from_be_bytes)
    .unwrap_or_default();
  RecoveryCert {
    account_id: ACCOUNT.to_owned(),
    old_epoch,
    new_epoch,
    new_device_pubkey: bytes.get(16..49).unwrap_or_default().to_vec(),
    attestation: bytes.get(49..).unwrap_or_default().to_vec(),
    signature,
  }
}

#[test]
fn recovery_cert_arbitrary_wire_fields_never_panic_or_mutate_on_rejection() {
  let root = key(1);
  let payment = key(2);
  let device = key(4);
  let mut mutated = RecoveryCert::sign(
    &root,
    ACCOUNT,
    0,
    device
      .verifying_key()
      .to_sec1_point(true)
      .as_bytes()
      .to_vec(),
    b"attestation".to_vec(),
  );
  let mutated_case = {
    let signed_bytes = mutated.signed_bytes();
    mutated.signature[0] ^= 1;
    (signed_bytes, mutated.signature)
  };
  let cert_cases = prop_oneof![
    (arbitrary_bytes(), arbitrary_bytes())
      .prop_map(|(bytes, signature)| (bytes, signature.into_iter().take(63).collect())),
    Just(mutated_case),
  ];

  runner()
    .run(&cert_cases, |(bytes, signature)| {
      let cert = candidate_cert(bytes, signature);
      let signed_bytes = cert.signed_bytes();
      let verification = keys::verify(root.verifying_key(), &signed_bytes, &cert.signature);
      let mut ledger = ledger(&root, &payment);
      let result = ledger.submit_recovery(cert, 0);
      prop_assert!(verification.is_err());
      prop_assert!(result.is_err());
      prop_assert_eq!(ledger.pending_request_no(ACCOUNT), None);
      prop_assert_eq!(ledger.epoch(ACCOUNT), Some(1));
      prop_assert_eq!(ledger.available(ACCOUNT, 0), Ok(1_000));
      Ok(())
    })
    .unwrap();

  let valid_cert = RecoveryCert::sign(
    &root,
    ACCOUNT,
    1,
    device
      .verifying_key()
      .to_sec1_point(true)
      .as_bytes()
      .to_vec(),
    b"attestation".to_vec(),
  );
  let mut valid_ledger = ledger(&root, &payment);
  valid_ledger.submit_recovery(valid_cert, 0).unwrap();
  assert_eq!(valid_ledger.pending_request_no(ACCOUNT), Some(1));
}

#[test]
fn recovery_cancel_arbitrary_bytes_never_panic_or_clear_pending_state_on_rejection() {
  let root = key(1);
  let payment = key(2);
  let device = key(4);
  let valid_cert = RecoveryCert::sign(
    &root,
    ACCOUNT,
    1,
    device
      .verifying_key()
      .to_sec1_point(true)
      .as_bytes()
      .to_vec(),
    b"attestation".to_vec(),
  );
  let mut invalid_signature = RecoveryCancel::sign(&payment, ACCOUNT, 1, &valid_cert, 1).signature;
  invalid_signature[0] ^= 1;
  let stale_epoch_signature = RecoveryCancel::sign(&payment, ACCOUNT, 1, &valid_cert, 1).signature;
  let cancel_cases = prop_oneof![
    arbitrary_bytes().prop_map(|signature| (signature.into_iter().take(63).collect(), 1)),
    Just((invalid_signature, 1)),
    Just((stale_epoch_signature, 2)),
  ];

  runner()
    .run(&cancel_cases, |(signature, epoch)| {
      let mut ledger = ledger(&root, &payment);
      ledger.submit_recovery(valid_cert.clone(), 0).unwrap();
      let cancel = RecoveryCancel {
        account_id: ACCOUNT.to_owned(),
        epoch,
        signature,
      };
      let result = ledger.cancel(&cancel, 1);
      prop_assert!(result.is_err(), "invalid cancel accepted for epoch {epoch}");
      prop_assert_eq!(ledger.pending_request_no(ACCOUNT), Some(1));
      prop_assert_eq!(ledger.epoch(ACCOUNT), Some(1));
      prop_assert_eq!(ledger.available(ACCOUNT, 1), Ok(1_000));
      Ok(())
    })
    .unwrap();
}
