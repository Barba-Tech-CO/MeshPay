#![no_main]
use libfuzzer_sys::fuzz_target;
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::cheque_stub::ChequeStub;
use meshpay_core::tx_wire::cheque_stub_body::ChequeStubBody;
use prost::Message;
use std::sync::OnceLock;

static ENGINE: OnceLock<PayerEngine> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
  let engine = ENGINE.get_or_init(|| {
    let path = tempfile::tempdir().unwrap().keep().join("payer.db");
    let payee = compressed_public_key(&signing_key_from_seed(&[9; 32]).unwrap());
    let engine = PayerEngine::open(&path, &[5; 32], &[7; 32], &payee).unwrap();
    let body = ChequeSpendBody {
      cheque_id: b"cheque-00000001".to_vec(),
      payee_account_id: b"payee-account".to_vec(),
      amount_units: 100,
      counter: 1,
      signed_at_unix: 1_900_000_000,
      ..Default::default()
    };
    let stub = ChequeStub {
      body: ChequeStubBody {
        cheque_id: body.cheque_id.clone(),
        expires_at_unix: 1_900_259_200,
        ..Default::default()
      }
      .encode_to_vec(),
      server_signature: vec![0; 64],
    };
    engine
      .propose(
        &body.encode_to_vec(),
        1,
        b"payee-account",
        &stub.encode_to_vec(),
      )
      .unwrap();
    engine
  });
  let _ = engine.on_ack(data);
});
