pub const CHEQUE_ID_LEN: usize = 16;
pub const CHEQUE_PUBLIC_KEY_LEN: usize = 33;
pub const BODY_LEN: usize = CHEQUE_ID_LEN + CHEQUE_PUBLIC_KEY_LEN + 8 + 8;

/// Message the holder gets blind-signed. The issuer never sees it, so EVERY
/// field is holder-chosen and unenforced by the issuer. `max_value_cents` and
/// `valid_until_unix` mean something only because the verifier compares them
/// with the values tied to the issuer key (one key per tier/window); see
/// `SignedStubToken::verify`. Account and epoch are deliberately absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubTokenBody {
  pub cheque_id: [u8; CHEQUE_ID_LEN],
  pub cheque_public_key: [u8; CHEQUE_PUBLIC_KEY_LEN],
  pub max_value_cents: u64,
  pub valid_until_unix: u64,
}

impl StubTokenBody {
  pub fn to_bytes(&self) -> [u8; BODY_LEN] {
    let mut out = [0u8; BODY_LEN];
    out[..16].copy_from_slice(&self.cheque_id);
    out[16..49].copy_from_slice(&self.cheque_public_key);
    out[49..57].copy_from_slice(&self.max_value_cents.to_be_bytes());
    out[57..].copy_from_slice(&self.valid_until_unix.to_be_bytes());
    out
  }
}
