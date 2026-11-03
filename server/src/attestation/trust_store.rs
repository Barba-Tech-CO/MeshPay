use std::collections::HashSet;
use x509_parser::pem::Pem;

/// Operator-supplied trust anchors (DER) and revoked certificate serials (lowercase hex).
/// KNOWN GAP: real Google roots and the attestation status list are not shipped or fetched; the
/// operator provides both files and keeps the revocation file current.
pub struct TrustStore {
  pub anchors: Vec<Vec<u8>>,
  pub revoked_serials: HashSet<String>,
}

impl TrustStore {
  pub fn load(roots_pem_file: &str, revocation_file: &str) -> Result<Self, String> {
    let pem_bytes = std::fs::read(roots_pem_file)
      .map_err(|e| format!("cannot read ATTESTATION_ROOTS_PEM_FILE: {e}"))?;
    let anchors = Pem::iter_from_buffer(&pem_bytes)
      .map(|pem| {
        pem
          .map(|p| p.contents)
          .map_err(|e| format!("ATTESTATION_ROOTS_PEM_FILE: {e}"))
      })
      .collect::<Result<Vec<_>, _>>()?;
    let revocations = std::fs::read_to_string(revocation_file)
      .map_err(|e| format!("cannot read ATTESTATION_REVOCATION_FILE: {e}"))?;
    let revoked_serials = revocations
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      // Serial 0 is a valid serial: keep it as "0", not the empty string.
      .map(|line| match line.trim_start_matches('0') {
        "" => "0".to_owned(),
        digits => digits.to_ascii_lowercase(),
      })
      .collect();
    Ok(Self {
      anchors,
      revoked_serials,
    })
  }
}
