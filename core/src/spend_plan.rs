use crate::tx_hashing::{body_with_tx_id, derive_tx_id};
use crate::tx_wire::cheque_spend_body::ChequeSpendBody;
use prost::Message;

/// A validated spend: decoded body, derived tx_id and the exact body bytes that get signed.
pub(crate) struct SpendPlan {
  pub(crate) body: ChequeSpendBody,
  pub(crate) tx_id: Vec<u8>,
  pub(crate) body_bytes: Vec<u8>,
}

impl SpendPlan {
  pub(crate) fn new(spend_body: &[u8], counter: u64, recipient_id: &[u8]) -> Result<Self, String> {
    let body = ChequeSpendBody::decode(spend_body).map_err(|e| format!("spend body: {e}"))?;
    if body.counter != counter {
      return Err("spend body counter differs from counter".to_owned());
    }
    if body.payee_account_id != recipient_id {
      return Err("spend body payee differs from recipient".to_owned());
    }
    let tx_id = derive_tx_id(counter, spend_body)?;
    if !body.tx_id.is_empty() && body.tx_id != tx_id {
      return Err("spend body tx_id does not match its content".to_owned());
    }
    let body_bytes = body_with_tx_id(spend_body, &tx_id)?;
    Ok(Self {
      body,
      tx_id,
      body_bytes,
    })
  }
}
