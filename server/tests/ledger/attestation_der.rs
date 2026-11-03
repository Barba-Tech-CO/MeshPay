//! KeyDescription DER parsing: malformed input never panics.
use crate::attestation_pki::KeySpec;
use meshpay_server::attestation::attestation_error::AttestationError;
use meshpay_server::attestation::key_description::KeyDescription;

struct XorShift(u64);

impl XorShift {
  fn next(&mut self) -> u64 {
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 7;
    self.0 ^= self.0 << 17;
    self.0
  }
}

#[test]
fn parses_a_valid_description() {
  let der = KeySpec::strongbox(b"abc").der();
  let parsed = KeyDescription::parse(&der).unwrap();
  assert_eq!(parsed.attestation_challenge, b"abc");
  assert_eq!(parsed.tee_enforced.origin, Some(0));
  assert_eq!(parsed.tee_enforced.ec_curve, Some(1));
  assert!(parsed.tee_enforced.root_of_trust.unwrap().device_locked);
}

#[test]
fn malformed_inputs_are_refused() {
  let der = KeySpec::strongbox(b"abc").der();
  for bad in [
    Vec::new(),
    vec![0x30],
    vec![0x30, 0x00],
    vec![0x04, 0x01, 0x00],
    [der.clone(), vec![0]].concat(),
    der[..der.len() - 1].to_vec(),
  ] {
    assert_eq!(
      KeyDescription::parse(&bad),
      Err(AttestationError::ExtensionMalformed)
    );
  }
}

#[test]
fn authorization_tags_must_be_context_specific() {
  // Application-class tag 1 carries the same number as the purposes field.
  let mut spec = KeySpec::strongbox(b"abc");
  spec.purposes_tag = 0x61;
  assert_eq!(
    KeyDescription::parse(&spec.der()),
    Err(AttestationError::ExtensionMalformed)
  );
}

#[test]
fn random_mutations_never_panic() {
  let original = KeySpec::strongbox(&[5u8; 32]).der();
  let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
  for _ in 0..5_000 {
    let mut bytes = original.clone();
    for _ in 0..=(rng.next() % 4) {
      let at = (rng.next() as usize) % bytes.len();
      match rng.next() % 4 {
        0 => bytes[at] = rng.next() as u8,
        1 => bytes[at] ^= 1 << (rng.next() % 8),
        2 => bytes.truncate(at.max(1)),
        _ => bytes.insert(at, rng.next() as u8),
      }
      if bytes.is_empty() {
        break;
      }
    }
    let _ = KeyDescription::parse(&bytes);
  }
}
