use crate::attestation::attestation_error::AttestationError;
use x509_parser::der_parser::ber::BerObject;
use x509_parser::der_parser::der::parse_der;

pub fn parse_whole(der: &[u8]) -> Result<BerObject<'_>, AttestationError> {
  match parse_der(der) {
    Ok(([], object)) => Ok(object),
    _ => Err(AttestationError::ExtensionMalformed),
  }
}

pub fn small_int(object: &BerObject) -> Result<u32, AttestationError> {
  object
    .as_u32()
    .map_err(|_| AttestationError::ExtensionMalformed)
}

pub fn seq<'a, 'b>(object: &'b BerObject<'a>) -> Result<&'b Vec<BerObject<'a>>, AttestationError> {
  object
    .as_sequence()
    .map_err(|_| AttestationError::ExtensionMalformed)
}

pub fn bytes<'b>(object: &'b BerObject) -> Result<&'b [u8], AttestationError> {
  object
    .as_slice()
    .map_err(|_| AttestationError::ExtensionMalformed)
}
