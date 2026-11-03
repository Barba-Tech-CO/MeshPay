use crate::attestation::attestation_error::AttestationError;
use crate::attestation::der_helpers::{parse_whole, seq, small_int};
use crate::attestation::root_of_trust::RootOfTrust;
use x509_parser::der_parser::ber::{BerObject, BerObjectContent, Class};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AuthorizationList {
  pub purposes: Vec<u32>,
  pub algorithm: Option<u32>,
  pub ec_curve: Option<u32>,
  pub origin: Option<u32>,
  pub root_of_trust: Option<RootOfTrust>,
}

impl AuthorizationList {
  pub fn parse(object: &BerObject) -> Result<Self, AttestationError> {
    let mut list = Self::default();
    for field in seq(object)? {
      // Every authorization tag is context-specific; a universal or application tag with the
      // same number must not be read as one.
      if field.header.class() != Class::ContextSpecific {
        return Err(AttestationError::ExtensionMalformed);
      }
      let BerObjectContent::Unknown(any) = &field.content else {
        return Err(AttestationError::ExtensionMalformed);
      };
      let inner = parse_whole(any.data)?;
      // Explicit tags: 1 purpose, 2 algorithm, 10 ecCurve, 702 origin, 704 rootOfTrust.
      match field.header.tag().0 {
        1 => {
          list.purposes = inner
            .as_set()
            .map_err(|_| AttestationError::ExtensionMalformed)?
            .iter()
            .map(small_int)
            .collect::<Result<_, _>>()?;
        }
        2 => list.algorithm = Some(small_int(&inner)?),
        10 => list.ec_curve = Some(small_int(&inner)?),
        702 => list.origin = Some(small_int(&inner)?),
        704 => list.root_of_trust = Some(RootOfTrust::parse(&inner)?),
        _ => {}
      }
    }
    Ok(list)
  }
}
