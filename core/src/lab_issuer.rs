//! LAB ONLY (TEST unit, no real value): the lab has no issuing server, so payers seal their own
//! stubs with a lab ISSUER key whose seed is public on purpose (anyone can mint lab stubs). The
//! lab root certifies it; a real build pins the operator root, which never certifies this key.
use crate::issuer_trust::IssuerTrust;
use crate::tx_keys::{compressed_public_key, sign, signing_key_from_seed};
use crate::tx_wire::cheque_stub::ChequeStub;
use crate::tx_wire::cheque_stub_body::ChequeStubBody;
use crate::tx_wire::server_key_cert::ServerKeyCert;
use crate::tx_wire::server_key_cert_body::ServerKeyCertBody;
use p256::ecdsa::SigningKey;
use prost::Message;
use sha2::{Digest, Sha256};

fn lab_key(label: &[u8]) -> SigningKey {
  signing_key_from_seed(&Sha256::digest(label)).expect("lab seed is a valid scalar")
}

fn root() -> SigningKey {
  lab_key(b"meshpay-lab-issuer-root-v1")
}

fn issuer() -> SigningKey {
  lab_key(b"meshpay-lab-issuer-key-v1")
}

pub fn root_public_key() -> Vec<u8> {
  compressed_public_key(&root())
}

/// Lab ISSUER `ServerKeyCert` valid for 30 days from `not_before_unix`.
pub fn cert(not_before_unix: u64) -> Vec<u8> {
  let pubkey = compressed_public_key(&issuer());
  let body = ServerKeyCertBody {
    key_id: IssuerTrust::key_id(&pubkey),
    pubkey,
    role: IssuerTrust::ISSUER_ROLE,
    not_before_unix,
    not_after_unix: not_before_unix.saturating_add(IssuerTrust::MAX_CERT_LIFETIME_SECS),
  }
  .encode_to_vec();
  ServerKeyCert {
    root_signature: sign(&root(), &IssuerTrust::cert_message(&body)),
    body,
  }
  .encode_to_vec()
}

/// Sealed `ChequeStub` bytes; `server_key_id` is set to the lab ISSUER key id.
pub fn seal_stub(body: ChequeStubBody) -> Vec<u8> {
  let body = ChequeStubBody {
    server_key_id: IssuerTrust::key_id(&compressed_public_key(&issuer())),
    ..body
  }
  .encode_to_vec();
  ChequeStub {
    server_signature: sign(&issuer(), &IssuerTrust::stub_message(&body)),
    body,
  }
  .encode_to_vec()
}
