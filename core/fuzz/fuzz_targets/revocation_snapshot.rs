#![no_main]
use libfuzzer_sys::fuzz_target;
use meshpay_core::dtn_buffer::DtnEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use std::sync::OnceLock;
use std::time::Duration;

static ENGINE: OnceLock<DtnEngine> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
  let engine = ENGINE.get_or_init(|| {
    let path = tempfile::tempdir().unwrap().keep().join("dtn.db");
    let key = compressed_public_key(&signing_key_from_seed(&[11; 32]).unwrap());
    DtnEngine::open_with_clock(
      &path,
      &[5; 32],
      &key,
      5 << 20,
      u32::MAX,
      Box::new(|| Duration::ZERO),
    )
    .unwrap()
  });
  let _ = engine.ingest_snapshot(data);
});
