//! Receiver-side check of the server seal on a `ChequeStub` (PRD 7.4, PV-SEC-05): the stub is
//! signed by an ISSUER key whose `ServerKeyCert` chains to the pinned offline root.
use crate::tx_keys::{verify, verifying_key_from_sec1};
use crate::tx_wire::cheque_stub::ChequeStub;
use crate::tx_wire::cheque_stub_body::ChequeStubBody;
use crate::tx_wire::server_key_cert::ServerKeyCert;
use crate::tx_wire::server_key_cert_body::ServerKeyCertBody;
use p256::ecdsa::VerifyingKey;
use prost::Message;
use sha2::{Digest, Sha256};

/// Verified ISSUER key: root signature, role, key id binding and lifetime already checked.
pub struct IssuerTrust {
  key: VerifyingKey,
  role: i32,
  key_id: Vec<u8>,
  not_before_unix: u64,
  not_after_unix: u64,
}

impl IssuerTrust {
  /// `ServerKeyRole::Issuer` wire value.
  pub const ISSUER_ROLE: i32 = 1;
  pub const SNAPSHOT_ROLE: i32 = 2;
  pub const MAX_CERT_LIFETIME_SECS: u64 = 30 * 86_400;

  /// Message the offline root signs: domain prefix plus the serialized `ServerKeyCertBody`.
  pub fn cert_message(cert_body: &[u8]) -> Vec<u8> {
    [b"meshpay/server-key-cert/v1".as_slice(), cert_body].concat()
  }

  /// Message the ISSUER key signs: domain prefix plus the serialized `ChequeStubBody`.
  pub fn stub_message(stub_body: &[u8]) -> Vec<u8> {
    [b"meshpay/stub/v1".as_slice(), stub_body].concat()
  }

  /// First 16 bytes of SHA-256 of the compressed public key (server key id).
  pub fn key_id(public_key: &[u8]) -> Vec<u8> {
    Sha256::digest(public_key)[..16].to_vec()
  }

  /// `root_public_key` is the pinned SEC1 root key; `cert` is a serialized `ServerKeyCert`.
  pub fn new(root_public_key: &[u8], cert: &[u8]) -> Result<Self, String> {
    Self::new_for_role(root_public_key, cert, Self::ISSUER_ROLE)
  }

  pub fn new_for_role(root_public_key: &[u8], cert: &[u8], role: i32) -> Result<Self, String> {
    let root = verifying_key_from_sec1(root_public_key)?;
    let cert = ServerKeyCert::decode(cert).map_err(|e| format!("server key cert: {e}"))?;
    verify(&root, &Self::cert_message(&cert.body), &cert.root_signature)
      .map_err(|_| "server key cert: bad root signature".to_owned())?;
    let body = ServerKeyCertBody::decode(cert.body.as_slice())
      .map_err(|_| "server key cert: bad body".to_owned())?;
    if body.role != role {
      return Err("server key cert: wrong role".to_owned());
    }
    if body.pubkey.len() != 33 || body.key_id != Self::key_id(&body.pubkey) {
      return Err("server key cert: key id does not match key".to_owned());
    }
    if body.not_after_unix <= body.not_before_unix
      || body.not_after_unix - body.not_before_unix > Self::MAX_CERT_LIFETIME_SECS
    {
      return Err("server key cert: lifetime outside 1s..=30 days".to_owned());
    }
    Ok(Self {
      key: verifying_key_from_sec1(&body.pubkey)?,
      role: body.role,
      key_id: body.key_id,
      not_before_unix: body.not_before_unix,
      not_after_unix: body.not_after_unix,
    })
  }

  pub fn public_key(&self) -> &VerifyingKey {
    &self.key
  }

  pub fn cert_window(&self) -> (u64, u64) {
    (self.not_before_unix, self.not_after_unix)
  }

  pub fn require_role(&self, role: i32) -> Result<(), String> {
    if self.role != role {
      return Err("server key cert: wrong role".to_owned());
    }
    Ok(())
  }

  pub fn check_snapshot_time(timestamp_unix: u64, cert_window: (u64, u64)) -> Result<(), String> {
    if timestamp_unix < cert_window.0 || timestamp_unix > cert_window.1 {
      return Err("snapshot issued_at outside server key cert window".to_owned());
    }
    Ok(())
  }

  /// Time-free part: seal signature, key id, and expiry within the cert lifetime. Returns the body.
  pub fn verify_stub(&self, stub: &ChequeStub) -> Result<ChequeStubBody, String> {
    verify(
      &self.key,
      &Self::stub_message(&stub.body),
      &stub.server_signature,
    )
    .map_err(|_| "stub server signature invalid".to_owned())?;
    let body =
      ChequeStubBody::decode(stub.body.as_slice()).map_err(|e| format!("stub body: {e}"))?;
    if body.server_key_id != self.key_id {
      return Err("stub sealed by an unknown server key".to_owned());
    }
    if body.expires_at_unix > self.not_after_unix {
      return Err("stub expires after its server key cert".to_owned());
    }
    Ok(body)
  }

  /// The ISSUER cert must be current at the protocol "now".
  pub fn check_current(&self, now_unix: u64) -> Result<(), String> {
    if now_unix < self.not_before_unix || now_unix > self.not_after_unix {
      return Err("server issuer key cert not current".to_owned());
    }
    Ok(())
  }
}
