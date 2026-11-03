use crate::wire::attestation_level::AttestationLevel;

/// Account trust level stored in the `attestation_level` enum column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "attestation_level", rename_all = "snake_case")]
pub enum AccountLevel {
  LabSoftware,
  SoftwareAttested,
  Tee,
  Strongbox,
}

impl AccountLevel {
  /// PRD 4.5: StrongBox issues; TEE only when the operator allows it; lab accounts only in lab
  /// mode; software-attested accounts only receive.
  pub fn may_issue(self, allow_tee: bool, lab_mode: bool) -> bool {
    match self {
      Self::Strongbox => true,
      Self::Tee => allow_tee,
      Self::LabSoftware => lab_mode,
      Self::SoftwareAttested => false,
    }
  }

  pub fn wire_level(self) -> AttestationLevel {
    match self {
      Self::Strongbox => AttestationLevel::AndroidStrongbox,
      Self::Tee => AttestationLevel::AndroidTee,
      Self::SoftwareAttested => AttestationLevel::None,
      Self::LabSoftware => AttestationLevel::LabSoftware,
    }
  }
}
