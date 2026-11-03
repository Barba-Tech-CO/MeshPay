/// Why an attestation was refused. Callers answer one uniform 403 and log only `code()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestationError {
  ChainEmpty,
  CertParse,
  ChainTooLong,
  ChainSignature,
  ChainNotCa,
  UntrustedRoot,
  CertExpired,
  CertRevoked,
  KeyMismatch,
  ExtensionMissing,
  ExtensionMalformed,
  ChallengeMismatch,
  ChallengeUnknown,
  ChallengeExpired,
  KeyProperties,
  BootState,
  SoftwareLevelNotAllowed,
  KeyAlgorithm,
}

impl AttestationError {
  pub fn code(self) -> &'static str {
    match self {
      Self::ChainEmpty => "att_chain_empty",
      Self::CertParse => "att_cert_parse",
      Self::ChainTooLong => "att_chain_too_long",
      Self::ChainSignature => "att_chain_signature",
      Self::ChainNotCa => "att_chain_not_ca",
      Self::UntrustedRoot => "att_untrusted_root",
      Self::CertExpired => "att_cert_expired",
      Self::CertRevoked => "att_cert_revoked",
      Self::KeyMismatch => "att_key_mismatch",
      Self::ExtensionMissing => "att_extension_missing",
      Self::ExtensionMalformed => "att_extension_malformed",
      Self::ChallengeMismatch => "att_challenge_mismatch",
      Self::ChallengeUnknown => "att_challenge_unknown",
      Self::ChallengeExpired => "att_challenge_expired",
      Self::KeyProperties => "att_key_properties",
      Self::BootState => "att_boot_state",
      Self::SoftwareLevelNotAllowed => "att_software_level_not_allowed",
      Self::KeyAlgorithm => "att_key_algorithm",
    }
  }
}
