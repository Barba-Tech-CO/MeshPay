mod common;

use blind_signatures::holder::PendingToken;
use blind_signatures::issuer::Issuer;
use blind_signatures::signed_stub_token::SignedStubToken;
use common::{new_pending, random_body, seeded_rng};

fn issue(issuer: &Issuer, seed: u64) -> SignedStubToken {
  let mut rng = seeded_rng(seed);
  let pending = new_pending(issuer, &mut rng);
  let blind_signature = issuer.sign_blinded(pending.blind_message()).unwrap();
  pending
    .finalize(issuer.public_key(), &blind_signature)
    .unwrap()
}

fn issuers() -> (Issuer, Issuer) {
  let mut rng = seeded_rng(1);
  (
    Issuer::generate(&mut rng, 2048).unwrap(),
    Issuer::generate(&mut rng, 2048).unwrap(),
  )
}

#[test]
fn roundtrip_verifies() {
  let (issuer, _) = issuers();
  let token = issue(&issuer, 10);
  token
    .verify(issuer.public_key(), 5_000, 1_800_000_000)
    .unwrap();
}

#[test]
fn same_body_blinded_twice_gives_different_requests_and_tokens() {
  let (issuer, _) = issuers();
  let mut rng = seeded_rng(11);
  let body = random_body(&mut rng);
  let a = PendingToken::blind(&mut rng, issuer.public_key(), body.clone()).unwrap();
  let b = PendingToken::blind(&mut rng, issuer.public_key(), body).unwrap();
  assert_ne!(a.blind_message(), b.blind_message());
  let sig_a = issuer.sign_blinded(a.blind_message()).unwrap();
  let sig_b = issuer.sign_blinded(b.blind_message()).unwrap();
  let ta = a.finalize(issuer.public_key(), &sig_a).unwrap();
  let tb = b.finalize(issuer.public_key(), &sig_b).unwrap();
  assert_ne!(ta.signature, tb.signature);
  ta.verify_signature(issuer.public_key()).unwrap();
  tb.verify_signature(issuer.public_key()).unwrap();
}

#[test]
fn tampered_body_fields_fail() {
  let (issuer, _) = issuers();
  let token = issue(&issuer, 12);
  let mut t = token.clone();
  t.body.cheque_id[0] ^= 1;
  assert!(t.verify_signature(issuer.public_key()).is_err());
  let mut t = token.clone();
  t.body.cheque_public_key[5] ^= 1;
  assert!(t.verify_signature(issuer.public_key()).is_err());
  let mut t = token.clone();
  t.body.max_value_cents += 1;
  assert!(t.verify_signature(issuer.public_key()).is_err());
  let mut t = token;
  t.body.valid_until_unix += 1;
  assert!(t.verify_signature(issuer.public_key()).is_err());
}

#[test]
fn tampered_signature_or_randomizer_fails() {
  let (issuer, _) = issuers();
  let token = issue(&issuer, 13);
  let mut t = token.clone();
  t.signature.0[100] ^= 1;
  assert!(t.verify_signature(issuer.public_key()).is_err());
  let mut t = token;
  t.randomizer.0[0] ^= 1;
  assert!(t.verify_signature(issuer.public_key()).is_err());
}

#[test]
fn wrong_key_fails_verify_and_finalize() {
  let (issuer, other) = issuers();
  let token = issue(&issuer, 14);
  assert!(token.verify_signature(other.public_key()).is_err());

  let mut rng = seeded_rng(15);
  let pending = new_pending(&issuer, &mut rng);
  let blind_signature = issuer.sign_blinded(pending.blind_message()).unwrap();
  assert!(
    pending
      .finalize(other.public_key(), &blind_signature)
      .is_err()
  );
}

#[test]
fn tampered_or_mismatched_blind_signature_fails_finalize() {
  let (issuer, _) = issuers();
  let mut rng = seeded_rng(16);
  let a = new_pending(&issuer, &mut rng);
  let b = new_pending(&issuer, &mut rng);
  let sig_a = issuer.sign_blinded(a.blind_message()).unwrap();
  let mut flipped = sig_a.clone();
  flipped.0[7] ^= 1;
  assert!(
    b.finalize(issuer.public_key(), &sig_a).is_err(),
    "signature of another request"
  );
  assert!(
    a.finalize(issuer.public_key(), &flipped).is_err(),
    "bit flip"
  );
}

#[test]
fn holder_chosen_u64_max_value_is_not_a_limit_unless_verifier_supplies_expected_values() {
  let (issuer, _) = issuers();
  let mut rng = seeded_rng(17);
  let mut body = random_body(&mut rng);
  body.max_value_cents = u64::MAX;
  let pending = PendingToken::blind(&mut rng, issuer.public_key(), body).unwrap();
  let blind_signature = issuer.sign_blinded(pending.blind_message()).unwrap();
  let token = pending
    .finalize(issuer.public_key(), &blind_signature)
    .unwrap();
  // The issuer signed it: signature alone accepts the absurd value.
  token.verify_signature(issuer.public_key()).unwrap();
  // Only the verifier's expected tier values reject it.
  assert!(
    token
      .verify(issuer.public_key(), 5_000, 1_800_000_000)
      .is_err()
  );
}

#[test]
fn issuer_rejects_wrong_length_blind_message() {
  let (issuer, _) = issuers();
  let bad = blind_rsa_signatures::BlindMessage(vec![1u8; 10]);
  assert!(issuer.sign_blinded(&bad).is_err());
}
