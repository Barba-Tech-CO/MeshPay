#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum ConflictKind {
  Unspecified = 0,
  SameCheque = 1,
  SameCounter = 2,
}
