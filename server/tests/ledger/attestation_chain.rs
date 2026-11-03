//! Chain verification without a database: every refusal has its own internal code.
use crate::attestation_pki::{KeySpec, Pki};
use crate::support::random_bytes;
use meshpay_server::attestation::account_level::AccountLevel;
use meshpay_server::attestation::attestation_error::AttestationError;
use meshpay_server::attestation::chain_verifier::verify_chain;
use meshpay_server::attestation::trust_store::TrustStore;
use meshpay_server::crypto::p256_keys::{compressed_public_key, signing_key_from_seed};
use p256::ecdsa::SigningKey;
use rcgen::{KeyPair, PKCS_ECDSA_P384_SHA384, PublicKeyData, SubjectPublicKeyInfo, date_time_ymd};
use std::collections::HashSet;

const NOW: i64 = 1_900_000_000;

type Change = fn(&mut KeySpec);

struct Fixture {
  pki: Pki,
  store: TrustStore,
  key: SigningKey,
  challenge: Vec<u8>,
}

impl Fixture {
  fn new() -> Self {
    let pki = Pki::new();
    let store = TrustStore {
      anchors: vec![pki.root_der.clone()],
      revoked_serials: HashSet::new(),
    };
    Self {
      pki,
      store,
      key: signing_key_from_seed(&random_bytes(32)).unwrap(),
      challenge: random_bytes(32),
    }
  }

  fn registered(&self) -> Vec<u8> {
    compressed_public_key(&self.key)
  }

  fn verify(&self, chain: &[Vec<u8>]) -> Result<AccountLevel, AttestationError> {
    verify_chain(chain, &self.store, NOW, &self.registered(), &self.challenge)
  }

  fn ok_chain(&self, spec: &KeySpec) -> Vec<Vec<u8>> {
    self.pki.valid_chain(self.key.verifying_key(), spec)
  }
}

#[test]
fn levels_follow_the_certificate_not_the_client() {
  let f = Fixture::new();
  let cases = [
    (KeySpec::strongbox(&f.challenge), AccountLevel::Strongbox),
    (KeySpec::tee(&f.challenge), AccountLevel::Tee),
    (
      KeySpec::software(&f.challenge),
      AccountLevel::SoftwareAttested,
    ),
  ];
  for (spec, level) in cases {
    assert_eq!(f.verify(&f.ok_chain(&spec)), Ok(level));
  }
  // StrongBox attestation over a TEE keymaster is only TEE.
  let mut mixed = KeySpec::strongbox(&f.challenge);
  mixed.keymaster_level = 1;
  assert_eq!(f.verify(&f.ok_chain(&mixed)), Ok(AccountLevel::Tee));
}

#[test]
fn leaf_key_must_be_ec_p256() {
  let f = Fixture::new();
  let p384 = KeyPair::generate_for(&PKCS_ECDSA_P384_SHA384).unwrap();
  let spki = SubjectPublicKeyInfo::from_der(&p384.subject_public_key_info()).unwrap();
  let chain = f.pki.chain_for_spki(
    &spki,
    Some(KeySpec::strongbox(&f.challenge).der()),
    7,
    date_time_ymd(2098, 1, 1),
  );
  assert_eq!(f.verify(&chain), Err(AttestationError::KeyAlgorithm));
}

#[test]
fn out_of_range_security_level_is_malformed() {
  let f = Fixture::new();
  let mut spec = KeySpec::strongbox(&f.challenge);
  spec.keymaster_level = 3;
  assert_eq!(
    f.verify(&f.ok_chain(&spec)),
    Err(AttestationError::ExtensionMalformed)
  );
}

#[test]
fn serial_zero_stays_revocable() {
  let dir = std::env::temp_dir().join(format!("meshpay-rev-{}", hex::encode(random_bytes(8))));
  std::fs::create_dir_all(&dir).unwrap();
  let revoked = dir.join("revoked.txt");
  std::fs::write(&revoked, "000\n\n").unwrap();
  let mut f = Fixture::new();
  f.store = TrustStore::load("/dev/null", revoked.to_str().unwrap()).unwrap();
  f.store.anchors = vec![f.pki.root_der.clone()];
  let spec = KeySpec::strongbox(&f.challenge);
  let chain = f.pki.chain(
    f.key.verifying_key(),
    Some(spec.der()),
    0,
    date_time_ymd(2098, 1, 1),
  );
  assert_eq!(f.verify(&chain), Err(AttestationError::CertRevoked));
}

#[test]
fn issuing_rights_follow_prd_4_5() {
  assert!(AccountLevel::Strongbox.may_issue(false, false));
  assert!(!AccountLevel::Tee.may_issue(false, false));
  assert!(AccountLevel::Tee.may_issue(true, false));
  assert!(!AccountLevel::SoftwareAttested.may_issue(true, true));
  assert!(!AccountLevel::LabSoftware.may_issue(true, false));
  assert!(AccountLevel::LabSoftware.may_issue(false, true));
}

#[test]
fn wrong_challenge() {
  let f = Fixture::new();
  let chain = f.ok_chain(&KeySpec::strongbox(&random_bytes(32)));
  assert_eq!(f.verify(&chain), Err(AttestationError::ChallengeMismatch));
}

#[test]
fn key_mismatch_with_registered_key() {
  let f = Fixture::new();
  let other = signing_key_from_seed(&random_bytes(32)).unwrap();
  let chain = f
    .pki
    .valid_chain(other.verifying_key(), &KeySpec::strongbox(&f.challenge));
  assert_eq!(f.verify(&chain), Err(AttestationError::KeyMismatch));
}

#[test]
fn broken_signature_in_chain() {
  let f = Fixture::new();
  let mut chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  let last = chain[1].len() - 1;
  chain[1][last] ^= 0x01;
  assert_eq!(f.verify(&chain), Err(AttestationError::ChainSignature));
  let mut chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  let last = chain[0].len() - 1;
  chain[0][last] ^= 0x01;
  assert_eq!(f.verify(&chain), Err(AttestationError::ChainSignature));
}

#[test]
fn untrusted_root() {
  let f = Fixture::new();
  let rogue = Pki::new();
  let chain = rogue.valid_chain(f.key.verifying_key(), &KeySpec::strongbox(&f.challenge));
  assert_eq!(f.verify(&chain), Err(AttestationError::UntrustedRoot));
  // Chain cut before the root is still anchored by the configured root.
  let mut cut = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  cut.pop();
  assert_eq!(f.verify(&cut), Ok(AccountLevel::Strongbox));
  let empty = TrustStore {
    anchors: Vec::new(),
    revoked_serials: HashSet::new(),
  };
  let chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  assert_eq!(
    verify_chain(&chain, &empty, NOW, &f.registered(), &f.challenge),
    Err(AttestationError::UntrustedRoot)
  );
}

#[test]
fn expired_certificate() {
  let f = Fixture::new();
  let spec = KeySpec::strongbox(&f.challenge);
  let chain = f.pki.chain(
    f.key.verifying_key(),
    Some(spec.der()),
    7,
    date_time_ymd(2021, 1, 1),
  );
  assert_eq!(f.verify(&chain), Err(AttestationError::CertExpired));
}

#[test]
fn missing_extension() {
  let f = Fixture::new();
  let chain = f
    .pki
    .chain(f.key.verifying_key(), None, 7, date_time_ymd(2098, 1, 1));
  assert_eq!(f.verify(&chain), Err(AttestationError::ExtensionMissing));
}

#[test]
fn revoked_serial() {
  let mut f = Fixture::new();
  f.store.revoked_serials.insert("1092".to_owned());
  // 4242 decimal is 0x1092.
  let chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  assert_eq!(f.verify(&chain), Err(AttestationError::CertRevoked));
}

#[test]
fn key_properties_and_boot_state() {
  let f = Fixture::new();
  let mutate: [(Change, AttestationError); 6] = [
    (|s| s.purposes = vec![3], AttestationError::KeyProperties),
    (|s| s.algorithm = 1, AttestationError::KeyProperties),
    (|s| s.curve = 2, AttestationError::KeyProperties),
    (|s| s.origin = 2, AttestationError::KeyProperties),
    (|s| s.boot = Some((false, 0)), AttestationError::BootState),
    (|s| s.boot = Some((true, 2)), AttestationError::BootState),
  ];
  for (change, expected) in mutate {
    let mut spec = KeySpec::strongbox(&f.challenge);
    change(&mut spec);
    assert_eq!(f.verify(&f.ok_chain(&spec)), Err(expected));
  }
  let mut no_root = KeySpec::tee(&f.challenge);
  no_root.boot = None;
  assert_eq!(
    f.verify(&f.ok_chain(&no_root)),
    Err(AttestationError::BootState)
  );
}

#[test]
fn empty_and_garbage_chains() {
  let f = Fixture::new();
  assert_eq!(f.verify(&[]), Err(AttestationError::ChainEmpty));
  assert_eq!(
    f.verify(&[random_bytes(40)]),
    Err(AttestationError::CertParse)
  );
  let long = vec![f.ok_chain(&KeySpec::strongbox(&f.challenge))[0].clone(); 9];
  assert_eq!(f.verify(&long), Err(AttestationError::ChainTooLong));
}

fn expired_ca_fixture(root_not_after_year: i32, intermediate_not_after_year: i32) -> Fixture {
  let mut f = Fixture::new();
  f.pki = Pki::with_ca_expiry(
    date_time_ymd(root_not_after_year, 1, 1),
    date_time_ymd(intermediate_not_after_year, 1, 1),
  );
  f.store.anchors = vec![f.pki.root_der.clone()];
  f
}

#[test]
fn expired_anchor_included_in_chain_is_accepted() {
  let f = expired_ca_fixture(2025, 2099);
  let chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  assert_eq!(chain.len(), 3);
  assert_eq!(f.verify(&chain), Ok(AccountLevel::Strongbox));
  let mut cut = chain;
  cut.pop();
  assert_eq!(f.verify(&cut), Ok(AccountLevel::Strongbox));
}

#[test]
fn expired_non_anchor_intermediate_is_refused() {
  let f = expired_ca_fixture(2099, 2025);
  let chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  assert_eq!(f.verify(&chain), Err(AttestationError::CertExpired));
}

#[test]
fn expired_root_not_configured_is_refused() {
  let mut f = expired_ca_fixture(2025, 2099);
  f.store.anchors = vec![Pki::new().root_der];
  let chain = f.ok_chain(&KeySpec::strongbox(&f.challenge));
  assert_eq!(f.verify(&chain), Err(AttestationError::CertExpired));
}
