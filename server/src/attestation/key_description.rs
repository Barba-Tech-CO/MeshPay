//! Android KeyDescription (OID 1.3.6.1.4.1.11129.2.1.17), parsed with `der-parser`.
use crate::attestation::attestation_error::AttestationError;
use crate::attestation::authorization_list::AuthorizationList;
use crate::attestation::der_helpers::{bytes, parse_whole, seq, small_int};

pub const SECURITY_LEVEL_SOFTWARE: u32 = 0;
pub const SECURITY_LEVEL_TEE: u32 = 1;
pub const SECURITY_LEVEL_STRONGBOX: u32 = 2;
/// KeyMint tag values the key must carry: purpose SIGN, algorithm EC, curve P-256, origin
/// GENERATED, and Verified boot state VERIFIED.
pub const PURPOSE_SIGN: u32 = 2;
pub const ALGORITHM_EC: u32 = 3;
pub const EC_CURVE_P256: u32 = 1;
pub const ORIGIN_GENERATED: u32 = 0;
pub const VERIFIED_BOOT_VERIFIED: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDescription {
  pub attestation_security_level: u32,
  pub keymaster_security_level: u32,
  pub attestation_challenge: Vec<u8>,
  pub software_enforced: AuthorizationList,
  pub tee_enforced: AuthorizationList,
}

impl KeyDescription {
  /// Strict DER, no trailing bytes; any deviation is `ExtensionMalformed`. Never panics.
  pub fn parse(der: &[u8]) -> Result<Self, AttestationError> {
    let root = parse_whole(der)?;
    let items = seq(&root)?;
    if items.len() < 8 {
      return Err(AttestationError::ExtensionMalformed);
    }
    Ok(Self {
      attestation_security_level: small_int(&items[1])?,
      keymaster_security_level: small_int(&items[3])?,
      attestation_challenge: bytes(&items[4])?.to_vec(),
      software_enforced: AuthorizationList::parse(&items[6])?,
      tee_enforced: AuthorizationList::parse(&items[7])?,
    })
  }
}
