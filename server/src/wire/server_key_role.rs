#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum ServerKeyRole {
  Unspecified = 0,
  Issuer = 1,
  Snapshot = 2,
  Tls = 3,
}
