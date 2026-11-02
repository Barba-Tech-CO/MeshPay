use meshpay_core::tx_hashing::body_with_tx_id;
use meshpay_core::tx_wire::ack_receipt::AckReceipt;
use meshpay_core::tx_wire::cheque_spend::ChequeSpend;
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::tx_propose::TxPropose;
use prost::Message;

/// Same literal as `app/test/core/tx/tx_wire_golden_test.dart`: the Dart hand encoder
/// and the prost decoder must agree on these bytes.
const DART_SPEND_BODY_HEX: &str =
  "0a100102030405060708090a0b0c0d0e0f101208a0a1a2a3a4a5a6a71887ad4b28053880e2cfaa06";
const PROPOSE_HEX: &str = "12520a4a0a100102030405060708090a0b0c0d0e0f101208a0a1a2a3a4a5a6a71887ad4b28053880e2cfaa062220000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f1204eeeeeeee";
const ACK_HEX: &str =
  "0a20000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f1204bbbbbbbb1a04cccccccc";

fn to_hex(bytes: &[u8]) -> String {
  bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Vec<u8> {
  (0..text.len())
    .step_by(2)
    .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
    .collect()
}

fn tx_id() -> Vec<u8> {
  (0..32).collect()
}

fn expected_body() -> ChequeSpendBody {
  ChequeSpendBody {
    cheque_id: (1..=16).collect(),
    payee_account_id: (0xa0..0xa8).collect(),
    amount_units: 1_234_567,
    tx_id: Vec::new(),
    counter: 5,
    prev_spend_hash: Vec::new(),
    signed_at_unix: 1_700_000_000,
  }
}

fn propose() -> TxPropose {
  let body = body_with_tx_id(&from_hex(DART_SPEND_BODY_HEX), &tx_id()).unwrap();
  TxPropose {
    spend: Some(ChequeSpend {
      body,
      payer_signature: vec![0xee; 4],
    }),
  }
}

fn ack() -> AckReceipt {
  AckReceipt {
    tx_id: tx_id(),
    spend_hash: vec![0xbb; 4],
    payee_signature: vec![0xcc; 4],
  }
}

#[test]
fn dart_spend_body_decodes_to_the_fixed_fields_and_reencodes_identically() {
  let bytes = from_hex(DART_SPEND_BODY_HEX);
  let body = ChequeSpendBody::decode(bytes.as_slice()).unwrap();
  assert_eq!(body, expected_body());
  assert_eq!(body.encode_to_vec(), bytes);
}

#[test]
fn propose_and_ack_match_the_literals_the_dart_parser_reads() {
  assert_eq!(to_hex(&propose().encode_to_vec()), PROPOSE_HEX);
  assert_eq!(to_hex(&ack().encode_to_vec()), ACK_HEX);
}
