#![no_main]
use libfuzzer_sys::fuzz_target;
use meshpay_core::acceptance_context::AcceptanceContext;
use meshpay_core::protocol_now::ProtocolNow;
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
  let context = AcceptanceContext {
    now: ProtocolNow {
      unix: 1_900_000_000,
      t_srv_unix: 1_900_000_000,
      rebooted: false,
    },
    snapshot_issued_at_unix: Some(1_900_000_000),
    expiry_secs: 72 * 3600,
  };
  let _ = engine.on_propose(data, &context);
});
