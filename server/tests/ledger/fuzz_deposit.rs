//! PV-SEC-03: fuzz the server's `DepositRequest` counterpart to PRD `RedeemRequest` and
//! `RevocationSnapshot` protobuf surfaces.
//! CI runs 1,000 cases per surface; set `MESHPAY_FUZZ_CASES=1000000` for the full PRD corpus.
//! Run with `MESHPAY_FUZZ_CASES=1000000 cargo test --release --test ledger fuzz -- --nocapture`.
use crate::support::{Client, TestApp, random_bytes, setup_isolated, setup_isolated_with};
use axum::http::StatusCode;
use meshpay_server::crypto::p256_keys::{compressed_public_key, key_id};
use meshpay_server::wire::deposit_request::DepositRequest;
use meshpay_server::wire::deposit_response::DepositResponse;
use meshpay_server::wire::kill_switch_request::KillSwitchRequest;
use meshpay_server::wire::redeem_status::RedeemStatus;
use meshpay_server::wire::revocation_snapshot::RevocationSnapshot;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use prost::Message;
use tokio::runtime::Builder;

fn runner() -> TestRunner {
  let cases = std::env::var("MESHPAY_FUZZ_CASES")
    .ok()
    .map(|value| {
      value
        .parse()
        .expect("MESHPAY_FUZZ_CASES must be a positive integer")
    })
    .unwrap_or(1_000);
  assert!(cases > 0, "MESHPAY_FUZZ_CASES must be positive");
  TestRunner::new(Config {
    cases,
    failure_persistence: None,
    ..Config::default()
  })
}

fn input(golden: Vec<u8>) -> impl Strategy<Value = Vec<u8>> {
  let length = golden.len();
  let wrong_signature = {
    let mut request = DepositRequest::decode(golden.as_slice()).unwrap();
    let spend = request.spend.as_mut().unwrap();
    spend.payer_signature[0] ^= 1;
    request.encode_to_vec()
  };
  prop_oneof![
    prop::collection::vec(any::<u8>(), 0..65536),
    (0..length, 0u8..8).prop_map({
      let golden = golden.clone();
      move |(index, bit)| {
        let mut bytes = golden.clone();
        bytes[index] ^= 1 << bit;
        bytes
      }
    }),
    (0..length).prop_map({
      let golden = golden.clone();
      move |cut| golden[..cut].to_vec()
    }),
    Just(wrong_signature),
    Just(golden.clone()).prop_map(|mut bytes| {
      bytes.extend_from_within(..);
      bytes
    }),
    Just(vec![0x0a, 0xff, 0xff, 0xff, 0xff, 0x7f]),
  ]
}

fn snapshot_input(seed: Vec<u8>) -> impl Strategy<Value = Vec<u8>> {
  let length = seed.len();
  prop_oneof![
    prop::collection::vec(any::<u8>(), 0..65536),
    (0..length, 0u8..8).prop_map({
      let seed = seed.clone();
      move |(index, bit)| {
        let mut bytes = seed.clone();
        bytes[index] ^= 1 << bit;
        bytes
      }
    }),
    (0..length).prop_map({
      let seed = seed.clone();
      move |cut| seed[..cut].to_vec()
    }),
    Just(seed.clone()).prop_map(|mut bytes| {
      bytes.extend_from_within(..);
      bytes
    }),
    Just(vec![0x0a, 0xff, 0xff, 0xff, 0xff, 0x7f]),
  ]
}

async fn ledger_counts(app: &TestApp) -> (i64, i64, i64, i64, i64, i64, i64, i64, i64) {
  sqlx::query_as(
    "SELECT (SELECT count(*) FROM accounts),
            (SELECT count(*) FROM balances),
            (SELECT count(*) FROM ledger_entries),
            (SELECT count(*) FROM chequebooks),
            (SELECT count(*) FROM cheques),
            (SELECT count(*) FROM cheque_releases),
            (SELECT count(*) FROM redemptions),
            (SELECT count(*) FROM equivocation_proofs),
            (SELECT count(*) FROM audit_log)",
  )
  .fetch_one(&app.owner_pool)
  .await
  .unwrap()
}

async fn control_counts(app: &TestApp) -> (i64, i64, i64, i64, i64) {
  sqlx::query_as(
    "SELECT (SELECT count(*) FROM server_key_certs),
            (SELECT count(*) FROM revoked_server_keys),
            (SELECT count(*) FROM issuance_freezes),
            (SELECT count(*) FROM audit_head),
            (SELECT count(*) FROM audit_log)",
  )
  .fetch_one(&app.owner_pool)
  .await
  .unwrap()
}

#[test]
fn deposit_bytes_never_panic_or_5xx_and_rejections_do_not_write_ledger_rows() {
  let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
  let Some(app) = runtime.block_on(setup_isolated(1_000_000, 10_000)) else {
    return;
  };
  let (payer, payee) = (Client::new(), Client::new());
  assert_eq!(runtime.block_on(payer.register(&app)).0, StatusCode::OK);
  assert_eq!(runtime.block_on(payee.register(&app)).0, StatusCode::OK);
  let (issue_status, issue_body) =
    runtime.block_on(payer.issue(&app, random_bytes(16), 1, 100, 60));
  assert_eq!(issue_status, StatusCode::OK);
  let book = issue_body.unwrap();
  let stub = book.stubs.first().unwrap();
  let spend = payer.spend(stub, &payee.account_id, 40, 1);
  let golden = DepositRequest {
    stub: Some(stub.clone()),
    spend: Some(spend),
  }
  .encode_to_vec();

  runner()
    .run(&input(golden), |bytes| {
      let before = runtime.block_on(ledger_counts(&app));
      let (status, body) = runtime.block_on(app.call("POST", "/v1/deposits", bytes));
      prop_assert!(status.as_u16() < 500, "unexpected server error: {status}");
      let rejected = if status == StatusCode::OK {
        let response = DepositResponse::decode(body.as_slice()).unwrap();
        !matches!(
          RedeemStatus::try_from(response.status),
          Ok(
            RedeemStatus::Paid
              | RedeemStatus::AlreadyRedeemedSameBody
              | RedeemStatus::RejectedConflict
          )
        )
      } else {
        true
      };
      if rejected {
        prop_assert_eq!(runtime.block_on(ledger_counts(&app)), before);
      }
      Ok(())
    })
    .unwrap();

  runtime.block_on(app.drop_isolated());
}

#[test]
fn revocation_snapshot_bytes_never_panic_or_5xx_or_write_rows() {
  let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
  let Some(app) = runtime.block_on(setup_isolated_with(1_000_000, 10_000, |config| {
    config.control_limit_per_minute = u32::MAX
  })) else {
    return;
  };
  let before = runtime.block_on(control_counts(&app));
  let root_public_key = compressed_public_key(&app.root_key);
  let seed = RevocationSnapshot {
    version: 1,
    issued_at_unix: 1,
    signer_key_id: key_id(&root_public_key),
    server_signature: vec![0; 64],
    ..Default::default()
  }
  .encode_to_vec();

  runner()
    .run(&snapshot_input(seed), |snapshot| {
      let request = KillSwitchRequest {
        snapshot,
        signer_cert: None,
      };
      let (status, _) =
        runtime.block_on(app.call("POST", "/v1/control/snapshot", request.encode_to_vec()));
      prop_assert!(status.as_u16() < 500, "unexpected server error: {status}");
      prop_assert_ne!(
        status,
        StatusCode::NO_CONTENT,
        "invalid snapshot was accepted"
      );
      Ok(())
    })
    .unwrap();

  assert_eq!(runtime.block_on(control_counts(&app)), before);
  runtime.block_on(app.drop_isolated());
}
