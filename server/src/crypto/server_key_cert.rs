//! `ServerKeyCert` (PRD 7.4): the offline root certifies each online key for one role, for at most
//! 30 days. The verifier always checks the role.
use crate::crypto::p256_keys::{
  COMPRESSED_PUBLIC_KEY_LEN, compressed_public_key, key_id, sign, verify,
};
use crate::crypto::signing_messages::server_key_cert_message;
use crate::wire::server_key_cert::ServerKeyCert;
use crate::wire::server_key_cert_body::ServerKeyCertBody;
use crate::wire::server_key_role::ServerKeyRole;
use p256::ecdsa::SigningKey;
use prost::Message;
use std::fmt;

pub const SECONDS_PER_DAY: u64 = 86_400;
pub const MAX_CERT_LIFETIME_SECONDS: u64 = 30 * SECONDS_PER_DAY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertError {
  BadRootSignature,
  BadBody,
  WrongRole,
  KeyIdMismatch,
  BadLifetime,
}

impl CertError {
  pub fn reason(self) -> &'static str {
    match self {
      Self::BadRootSignature => "server key cert: bad root signature",
      Self::BadBody => "server key cert: bad body",
      Self::WrongRole => "server key cert: wrong role",
      Self::KeyIdMismatch => "server key cert: key id does not match key",
      Self::BadLifetime => "server key cert: lifetime outside 1s..=30 days",
    }
  }
}

impl fmt::Display for CertError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.reason())
  }
}

/// Root-side signing (lab tool and tests; the real root never touches an online machine).
pub fn issue_cert(
  root: &SigningKey,
  key: &SigningKey,
  role: ServerKeyRole,
  not_before_unix: u64,
  not_after_unix: u64,
) -> ServerKeyCert {
  let pubkey = compressed_public_key(key);
  let body = ServerKeyCertBody {
    key_id: key_id(&pubkey),
    pubkey,
    role: role as i32,
    not_before_unix,
    not_after_unix,
  }
  .encode_to_vec();
  ServerKeyCert {
    root_signature: sign(root, &server_key_cert_message(&body)),
    body,
  }
}

/// Root signature, role, key id binding and lifetime <= 30 days (PRD 7.4 [ILU]). Time validity is `is_current`.
pub fn verify_cert(
  cert: &ServerKeyCert,
  root_public_key: &[u8],
  role: ServerKeyRole,
) -> Result<ServerKeyCertBody, CertError> {
  verify(
    root_public_key,
    &server_key_cert_message(&cert.body),
    &cert.root_signature,
  )
  .map_err(|_| CertError::BadRootSignature)?;
  let body = ServerKeyCertBody::decode(cert.body.as_slice()).map_err(|_| CertError::BadBody)?;
  if body.role != role as i32 {
    return Err(CertError::WrongRole);
  }
  if body.pubkey.len() != COMPRESSED_PUBLIC_KEY_LEN || body.key_id != key_id(&body.pubkey) {
    return Err(CertError::KeyIdMismatch);
  }
  if body.not_after_unix <= body.not_before_unix
    || body.not_after_unix - body.not_before_unix > MAX_CERT_LIFETIME_SECONDS
  {
    return Err(CertError::BadLifetime);
  }
  Ok(body)
}

pub fn is_current(body: &ServerKeyCertBody, now_unix: u64) -> bool {
  body.not_before_unix <= now_unix && now_unix <= body.not_after_unix
}
