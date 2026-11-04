//! PV-CRIP-05 randomized StrongBox issuance and spend limits.
use crate::attestation_pki::{KeySpec, Pki};
use crate::support::{Client, TestApp, random_bytes, setup_isolated_with};
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use meshpay_server::wire::attestation_level::AttestationLevel;
use meshpay_server::wire::challenge_response::ChallengeResponse;
use meshpay_server::wire::cheque_spend_body::ChequeSpendBody;
use meshpay_server::wire::cheque_stub_body::ChequeStubBody;
use meshpay_server::wire::redeem_status::RedeemStatus;
use prost::Message;
use std::path::PathBuf;
use std::time::Instant;

async fn strongbox_app() -> Option<(TestApp, Pki, PathBuf)> {
  let directory =
    std::env::temp_dir().join(format!("meshpay-cap-{}", hex::encode(random_bytes(8))));
  std::fs::create_dir(&directory).unwrap();
  let pki = Pki::new();
  let body = STANDARD.encode(&pki.root_der);
  let lines: Vec<&str> = body
    .as_bytes()
    .chunks(64)
    .map(|chunk| std::str::from_utf8(chunk).unwrap())
    .collect();
  let pem = format!(
    "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
    lines.join("\n")
  );
  let roots = directory.join("roots.pem");
  let revoked = directory.join("revoked.txt");
  std::fs::write(&roots, pem).unwrap();
  std::fs::write(&revoked, "").unwrap();

  let app = setup_isolated_with(1_000_000_000_000, 1_000_000_000, |config| {
    config.lab_mode = false;
    config.attestation_roots_pem_file = roots.to_string_lossy().into_owned();
    config.attestation_revocation_file = revoked.to_string_lossy().into_owned();
  })
  .await;
  let Some(app) = app else {
    std::fs::remove_dir_all(&directory).unwrap();
    return None;
  };
  Some((app, pki, directory))
}

async fn register_strongbox(app: &TestApp, pki: &Pki, client: &Client) {
  let (status, bytes) = app.call("GET", "/v1/challenge", Vec::new()).await;
  assert_eq!(status, StatusCode::OK);
  let challenge = ChallengeResponse::decode(bytes.as_slice()).unwrap().nonce;
  let chain = pki.valid_chain(&client.verifying_key(), &KeySpec::strongbox(&challenge));
  let request = client.attested_request(chain, challenge);
  assert_eq!(
    app
      .call("POST", "/v1/accounts", request.encode_to_vec())
      .await
      .0,
    StatusCode::OK
  );
}

fn random_below(max_exclusive: u64) -> u64 {
  assert!(max_exclusive > 0);
  u64::from_le_bytes(random_bytes(8).try_into().unwrap()) % max_exclusive
}

#[tokio::test]
async fn randomized_issuance_and_spends_stay_within_strongbox_caps() {
  let Some((app, pki, attestation_dir)) = strongbox_app().await else {
    return;
  };
  let payer = Client::new();
  let payee = Client::new();
  register_strongbox(&app, &pki, &payer).await;
  register_strongbox(&app, &pki, &payee).await;

  let cap_f = app.state.config.cap_cheque_units;
  let cap_r = app.state.config.cap_reserve_units;
  let n_max = u64::from(app.state.config.cheque_max_count.min(10));
  let mut counter = 0;
  let mut duplicate = None;

  let boundary_count = n_max;
  let boundary_amount = cap_r / boundary_count;
  assert!(boundary_amount > 0 && boundary_amount <= cap_f);
  let (status, boundary_book) = payer
    .issue(
      &app,
      random_bytes(16),
      boundary_count as u32,
      boundary_amount,
      60,
    )
    .await;
  assert_eq!(status, StatusCode::OK);
  let boundary_book = boundary_book.unwrap();
  assert_eq!(
    boundary_book.reserve_units,
    boundary_count * boundary_amount
  );
  assert_eq!(boundary_book.stubs.len(), boundary_count as usize);
  for stub in &boundary_book.stubs {
    let body = ChequeStubBody::decode(stub.body.as_slice()).unwrap();
    assert!(body.max_amount_units <= cap_f);
    assert_eq!(
      body.attestation_level,
      AttestationLevel::AndroidStrongbox as i32
    );
    assert!(!body.single_use_key);
    counter += 1;
    let spend = payer.spend(stub, &payee.account_id, body.max_amount_units, counter);
    assert_eq!(
      app.deposit(stub, &spend).await.status,
      RedeemStatus::Paid as i32
    );
    if duplicate.is_none() {
      duplicate = Some((stub.clone(), spend));
    }
  }

  let started = Instant::now();
  for round in 0..1_000 {
    let scenario = random_below(4);
    let (cheque_count, max_amount, should_issue) = match scenario {
      0 => (1, cap_f + 1, false),
      1 => (n_max as u32 + 1, 1, false),
      2 => {
        let count = cap_r / cap_f + 1;
        let amount = cap_r.div_ceil(count);
        if count <= n_max && amount <= cap_f {
          (count as u32, amount, false)
        } else {
          (1, cap_f + 1, false)
        }
      }
      _ => {
        let per_request_cap = (cap_r / 1_000).max(1).min(cap_f);
        let count_limit = n_max.min(per_request_cap);
        let count = 1 + random_below(count_limit);
        let amount_limit = cap_f.min(per_request_cap / count);
        (count as u32, 1 + random_below(amount_limit), true)
      }
    };
    let (status, book) = payer
      .issue(&app, random_bytes(16), cheque_count, max_amount, 60)
      .await;

    if !should_issue {
      assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "round {round}");
      assert!(book.is_none(), "round {round} returned an issued book");
      continue;
    }

    assert_eq!(status, StatusCode::OK, "round {round}");
    let book = book.unwrap();
    assert_eq!(book.stubs.len(), cheque_count as usize, "round {round}");
    assert_eq!(
      book.reserve_units,
      u64::from(cheque_count) * max_amount,
      "round {round}"
    );
    assert!(book.reserve_units <= cap_r, "round {round}");
    let reserved = app.balance(&payer.account_id).await.1;
    assert!(reserved >= 0 && reserved as u64 <= cap_r, "round {round}");

    for stub in &book.stubs {
      let body = ChequeStubBody::decode(stub.body.as_slice()).unwrap();
      assert!(body.max_amount_units <= cap_f, "round {round}");
      assert_eq!(
        body.attestation_level,
        AttestationLevel::AndroidStrongbox as i32,
        "round {round}"
      );
      assert!(!body.single_use_key, "round {round}");
    }

    let stub = &book.stubs[0];
    let body = ChequeStubBody::decode(stub.body.as_slice()).unwrap();
    counter += 1;
    let over_max = payer.spend(stub, &payee.account_id, body.max_amount_units + 1, counter);
    assert_eq!(
      app.deposit(stub, &over_max).await.status,
      RedeemStatus::RejectedInvalid as i32,
      "over-max spend in round {round}"
    );

    counter += 1;
    let amount = 1 + random_below(body.max_amount_units);
    let spend = payer.spend(stub, &payee.account_id, amount, counter);
    assert_eq!(
      app.deposit(stub, &spend).await.status,
      RedeemStatus::Paid as i32,
      "valid spend in round {round}"
    );
    if duplicate.is_none() {
      duplicate = Some((stub.clone(), spend));
    }
  }
  eprintln!(
    "PV-CRIP-05 randomized issuance: requests=1000, elapsed_ms={:.1}",
    started.elapsed().as_secs_f64() * 1_000.0
  );

  let (stub, first_spend) = duplicate.expect("boundary issuance provided a spent cheque");
  let first_body = ChequeSpendBody::decode(first_spend.body.as_slice()).unwrap();
  counter += 1;
  let second_spend = payer.spend(&stub, &payee.account_id, first_body.amount_units, counter);
  assert_eq!(
    app.deposit(&stub, &second_spend).await.status,
    RedeemStatus::RejectedConflict as i32
  );

  app.drop_isolated().await;
  std::fs::remove_dir_all(attestation_dir).unwrap();
}
