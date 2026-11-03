//! Third-party check of an `EquivocationProof` (PV-CRIP-01). Deliberately shares no code with the
//! server's verification path: only p256 + sha2 here; the wire structs are used just to decode bytes.
use meshpay_server::wire::cheque_spend_body::ChequeSpendBody;
use meshpay_server::wire::cheque_stub_body::ChequeStubBody;
use meshpay_server::wire::conflict_kind::ConflictKind;
use meshpay_server::wire::equivocation_proof::EquivocationProof;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use prost::Message;
use sha2::{Digest, Sha256};

pub struct IndependentVerifier;

impl IndependentVerifier {
  fn check_signature(key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), String> {
    let key = VerifyingKey::from_sec1_bytes(key).map_err(|e| format!("key: {e}"))?;
    let signature = Signature::from_slice(signature).map_err(|e| format!("sig: {e}"))?;
    key
      .verify(message, &signature)
      .map_err(|_| "bad signature".to_owned())
  }

  /// Verifies both spends and stubs, then the conflict itself, against `server_public_key`.
  pub fn verify(proof: &EquivocationProof, server_public_key: &[u8]) -> Result<(), String> {
    let (Some(stub_1), Some(spend_1), Some(stub_2), Some(spend_2)) =
      (&proof.stub_1, &proof.spend_1, &proof.stub_2, &proof.spend_2)
    else {
      return Err("incomplete proof".to_owned());
    };
    let mut bodies = Vec::new();
    for (stub, spend) in [(stub_1, spend_1), (stub_2, spend_2)] {
      let seal = [b"meshpay/stub/v1".as_slice(), &stub.body].concat();
      Self::check_signature(server_public_key, &seal, &stub.server_signature)?;
      let stub_body = ChequeStubBody::decode(stub.body.as_slice()).map_err(|e| e.to_string())?;
      let message = [b"meshpay/spend/v1".as_slice(), &spend.body].concat();
      Self::check_signature(&stub_body.signer_pubkey, &message, &spend.payer_signature)?;
      let spend_body = ChequeSpendBody::decode(spend.body.as_slice()).map_err(|e| e.to_string())?;
      if spend_body.cheque_id != stub_body.cheque_id {
        return Err("spend does not match its stub".to_owned());
      }
      if stub_body.payer_account_id != proof.offender_account_id
        || stub_body.payer_epoch != proof.epoch
      {
        return Err("stub is not the offender's in this epoch".to_owned());
      }
      bodies.push((stub_body, spend_body, Sha256::digest(&spend.body)));
    }
    let ((stub_a, body_a, hash_a), (stub_b, body_b, hash_b)) = (&bodies[0], &bodies[1]);
    if stub_a.signer_pubkey != stub_b.signer_pubkey {
      return Err("different signers".to_owned());
    }
    match ConflictKind::try_from(proof.conflict_kind) {
      Ok(ConflictKind::SameCheque) if body_a.cheque_id == body_b.cheque_id && hash_a != hash_b => {
        Ok(())
      }
      Ok(ConflictKind::SameCounter)
        if body_a.counter == body_b.counter && body_a.cheque_id != body_b.cheque_id =>
      {
        Ok(())
      }
      _ => Err("not a conflict".to_owned()),
    }
  }
}
