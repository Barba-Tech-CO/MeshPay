//! Every domain-separated message a key signs, in one place.

/// Message the payer signs: domain prefix plus the exact spend body bytes.
pub fn spend_signing_message(body_bytes: &[u8]) -> Vec<u8> {
  [b"meshpay/spend/v1".as_slice(), body_bytes].concat()
}

/// Message the server signs to seal a `ChequeStub`.
pub fn stub_seal_message(stub_body_bytes: &[u8]) -> Vec<u8> {
  [b"meshpay/stub/v1".as_slice(), stub_body_bytes].concat()
}

/// Proof of possession at registration.
pub fn register_message(public_key: &[u8]) -> Vec<u8> {
  [b"meshpay/register/v1".as_slice(), public_key].concat()
}

/// Account-signed chequebook request.
pub fn issue_message(request_body_bytes: &[u8]) -> Vec<u8> {
  [b"meshpay/issue/v1".as_slice(), request_body_bytes].concat()
}

/// Account-signed balance read at `timestamp` (unix seconds).
pub fn balance_message(account_id: &[u8], timestamp: u64) -> Vec<u8> {
  [
    b"meshpay/balance/v1".as_slice(),
    account_id,
    &timestamp.to_be_bytes(),
  ]
  .concat()
}

/// Message the offline root signs: a `ServerKeyCertBody`.
pub fn server_key_cert_message(cert_body_bytes: &[u8]) -> Vec<u8> {
  [b"meshpay/server-key-cert/v1".as_slice(), cert_body_bytes].concat()
}

/// Message a SNAPSHOT key (or the root) signs: the snapshot with `server_signature` empty.
/// Same bytes as `core::dtn_snapshot::snapshot_signing_message`.
pub fn snapshot_signing_message(unsigned_snapshot_bytes: &[u8]) -> Vec<u8> {
  [b"meshpay/snapshot/v1".as_slice(), unsigned_snapshot_bytes].concat()
}
