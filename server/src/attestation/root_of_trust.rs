use crate::attestation::attestation_error::AttestationError;
use crate::attestation::der_helpers::{seq, small_int};
use x509_parser::der_parser::ber::BerObject;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RootOfTrust {
  pub device_locked: bool,
  pub verified_boot_state: u32,
}

impl RootOfTrust {
  pub fn parse(object: &BerObject) -> Result<Self, AttestationError> {
    let items = seq(object)?;
    if items.len() < 3 {
      return Err(AttestationError::ExtensionMalformed);
    }
    Ok(Self {
      device_locked: items[1]
        .as_bool()
        .map_err(|_| AttestationError::ExtensionMalformed)?,
      verified_boot_state: small_int(&items[2])?,
    })
  }
}
