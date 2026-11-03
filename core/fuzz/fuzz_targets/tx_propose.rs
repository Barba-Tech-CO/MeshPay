#![no_main]
use libfuzzer_sys::fuzz_target;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use std::sync::OnceLock;

static ENGINE: OnceLock<ReceiverEngine> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
  let engine = ENGINE.get_or_init(|| {
    let path = tempfile::tempdir().unwrap().keep().join("receiver.db");
    let payer = compressed_public_key(&signing_key_from_seed(&[7; 32]).unwrap());
    ReceiverEngine::open(&path, &[5; 32], &[9; 32], &payer, b"payee-account").unwrap()
  });
  let _ = engine.on_propose(data);
});
