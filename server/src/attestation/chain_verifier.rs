use crate::attestation::account_level::AccountLevel;
use crate::attestation::attestation_error::AttestationError;
use crate::attestation::key_description::{
  ALGORITHM_EC, EC_CURVE_P256, KeyDescription, ORIGIN_GENERATED, PURPOSE_SIGN,
  SECURITY_LEVEL_STRONGBOX, SECURITY_LEVEL_TEE, VERIFIED_BOOT_VERIFIED,
};
use crate::attestation::trust_store::TrustStore;
use p256::ecdsa::VerifyingKey;
use x509_parser::certificate::X509Certificate;
use x509_parser::der_parser::oid::Oid;
use x509_parser::oid_registry::{OID_EC_P256, OID_KEY_TYPE_EC_PUBLIC_KEY};
use x509_parser::prelude::FromDer;
use x509_parser::time::ASN1Time;

/// Leaf, a few intermediates, and the root; Android chains are 3 to 5 long.
const MAX_CHAIN_LEN: usize = 8;

/// Verifies an Android Key Attestation chain (leaf first) against the trust store and returns the
/// account level it earns. The level comes only from the verified certificate, never from the client.
pub fn verify_chain(
  chain: &[Vec<u8>],
  store: &TrustStore,
  now_unix: i64,
  registered_key: &[u8],
  challenge: &[u8],
) -> Result<AccountLevel, AttestationError> {
  let certs = parse_chain(chain, store, now_unix)?;
  check_links(&certs, chain, store)?;
  let leaf = &certs[0];
  check_leaf_key(leaf, registered_key)?;

  let oid =
    Oid::from(&[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17]).map_err(|_| AttestationError::CertParse)?;
  let extension = leaf
    .get_extension_unique(&oid)
    .map_err(|_| AttestationError::ExtensionMalformed)?
    .ok_or(AttestationError::ExtensionMissing)?;
  let description = KeyDescription::parse(extension.value)?;
  if description.attestation_challenge != challenge {
    return Err(AttestationError::ChallengeMismatch);
  }
  level_of(&description)
}

/// Length, DER, revocation and validity window of every certificate.
fn parse_chain<'a>(
  chain: &'a [Vec<u8>],
  store: &TrustStore,
  now_unix: i64,
) -> Result<Vec<X509Certificate<'a>>, AttestationError> {
  if chain.is_empty() {
    return Err(AttestationError::ChainEmpty);
  }
  if chain.len() > MAX_CHAIN_LEN {
    return Err(AttestationError::ChainTooLong);
  }
  let certs = chain
    .iter()
    .map(|der| match X509Certificate::from_der(der) {
      Ok(([], cert)) => Ok(cert),
      _ => Err(AttestationError::CertParse),
    })
    .collect::<Result<Vec<_>, _>>()?;
  let now = ASN1Time::from_timestamp(now_unix).map_err(|_| AttestationError::CertParse)?;
  for cert in &certs {
    if store
      .revoked_serials
      .contains(&cert.serial.to_str_radix(16))
    {
      return Err(AttestationError::CertRevoked);
    }
    if !cert.validity().is_valid_at(now) {
      return Err(AttestationError::CertExpired);
    }
  }
  Ok(certs)
}

/// Each certificate is signed by a CA above it and the chain ends at a configured anchor.
fn check_links(
  certs: &[X509Certificate],
  chain: &[Vec<u8>],
  store: &TrustStore,
) -> Result<(), AttestationError> {
  for pair in certs.windows(2) {
    let is_ca = matches!(pair[1].basic_constraints(), Ok(Some(b)) if b.value.ca);
    if !is_ca {
      return Err(AttestationError::ChainNotCa);
    }
    if pair[0].issuer() != pair[1].subject()
      || pair[0]
        .verify_signature(Some(pair[1].public_key()))
        .is_err()
    {
      return Err(AttestationError::ChainSignature);
    }
  }
  let last = certs.len() - 1;
  if anchored(&certs[last], &chain[last], store) {
    Ok(())
  } else {
    Err(AttestationError::UntrustedRoot)
  }
}

/// The leaf must be an EC P-256 key equal to the key being registered.
fn check_leaf_key(leaf: &X509Certificate, registered_key: &[u8]) -> Result<(), AttestationError> {
  let spki = leaf.public_key();
  let curve = spki
    .algorithm
    .parameters
    .as_ref()
    .and_then(|parameters| parameters.as_oid().ok());
  if spki.algorithm.algorithm != OID_KEY_TYPE_EC_PUBLIC_KEY || curve != Some(OID_EC_P256) {
    return Err(AttestationError::KeyAlgorithm);
  }
  let attested = VerifyingKey::from_sec1_bytes(&spki.subject_public_key.data)
    .map_err(|_| AttestationError::KeyMismatch)?;
  let registered =
    VerifyingKey::from_sec1_bytes(registered_key).map_err(|_| AttestationError::KeyMismatch)?;
  if attested == registered {
    Ok(())
  } else {
    Err(AttestationError::KeyMismatch)
  }
}

/// The chain end is a configured anchor itself, or is signed by one.
fn anchored(last: &X509Certificate, last_der: &[u8], store: &TrustStore) -> bool {
  store.anchors.iter().any(|anchor_der| {
    anchor_der == last_der
      || X509Certificate::from_der(anchor_der).is_ok_and(|(_, anchor)| {
        last.issuer() == anchor.subject()
          && last.verify_signature(Some(anchor.public_key())).is_ok()
      })
  })
}

fn level_of(description: &KeyDescription) -> Result<AccountLevel, AttestationError> {
  if description.attestation_security_level > SECURITY_LEVEL_STRONGBOX
    || description.keymaster_security_level > SECURITY_LEVEL_STRONGBOX
  {
    return Err(AttestationError::ExtensionMalformed);
  }
  let level = description
    .attestation_security_level
    .min(description.keymaster_security_level);
  let hardware = level >= SECURITY_LEVEL_TEE;
  // Hardware-backed keys must carry their properties in the tee-enforced list.
  let list = if hardware {
    &description.tee_enforced
  } else {
    &description.software_enforced
  };
  let sound = list.purposes.contains(&PURPOSE_SIGN)
    && list.algorithm == Some(ALGORITHM_EC)
    && list.ec_curve == Some(EC_CURVE_P256)
    && list.origin == Some(ORIGIN_GENERATED);
  if !sound {
    return Err(AttestationError::KeyProperties);
  }
  if !hardware {
    return Ok(AccountLevel::SoftwareAttested);
  }
  // PV-SIL-02: locked bootloader and Verified boot state at key generation.
  match &list.root_of_trust {
    Some(root) if root.device_locked && root.verified_boot_state == VERIFIED_BOOT_VERIFIED => {}
    _ => return Err(AttestationError::BootState),
  }
  Ok(if level == SECURITY_LEVEL_STRONGBOX {
    AccountLevel::Strongbox
  } else {
    AccountLevel::Tee
  })
}
