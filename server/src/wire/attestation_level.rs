#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum AttestationLevel {
  Unspecified = 0,
  None = 1,
  IosAppAttest = 2,
  AndroidTee = 3,
  AndroidStrongbox = 4,
  /// Lab only: software key, accepted solely when lab_mode is on (PV-SEC-01).
  LabSoftware = 99,
}
