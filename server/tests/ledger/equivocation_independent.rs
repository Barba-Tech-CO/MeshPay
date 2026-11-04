//! PV-LED-02 / PV-CRIP-01: default 100 conflict proofs; set
//! `MESHPAY_EQUIVOCATION_ROUNDS=1000` for the full PRD scale. The 10,000-sequence honest test is
//! fixed.
use crate::independent_verifier::IndependentVerifier;
use crate::support::{Client, random_bytes, setup};
use meshpay_server::crypto::p256_keys::{
  compressed_public_key, key_id, sign, signing_key_from_seed,
};
use meshpay_server::crypto::signing_messages::{spend_signing_message, stub_seal_message};
use meshpay_server::crypto::spend_hashing::derive_tx_id;
use meshpay_server::wire::cheque_spend::ChequeSpend;
use meshpay_server::wire::cheque_spend_body::ChequeSpendBody;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::cheque_stub_body::ChequeStubBody;
use meshpay_server::wire::conflict_kind::ConflictKind;
use meshpay_server::wire::equivocation_proof::EquivocationProof;
use meshpay_server::wire::redeem_status::RedeemStatus;
use p256::ecdsa::SigningKey;
use prost::Message;
use std::time::{Duration, Instant};

#[tokio::test]
async fn double_deposit_proof_passes_independent_verifier() {
  let Some(app) = setup(true).await else { return };
  let rounds: usize = std::env::var("MESHPAY_EQUIVOCATION_ROUNDS")
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(100);
  let (first_payee, second_payee) = (Client::new(), Client::new());
  first_payee.register(&app).await;
  second_payee.register(&app).await;
  let server_key = app.state.public_key();
  let mut verifier_time = Duration::ZERO;
  let mut verifier_max = Duration::ZERO;
  for round in 0..rounds {
    let payer = Client::new();
    payer.register(&app).await;
    let (_, book) = payer.issue(&app, random_bytes(16), 1, 100, 60).await;
    let stub = book.unwrap().stubs.remove(0);
    let spend_a = payer.spend(&stub, &first_payee.account_id, 40, 1);
    let spend_b = payer.spend(&stub, &second_payee.account_id, 50, 1);
    let paid = app.deposit(&stub, &spend_a).await;
    assert_eq!(paid.status, RedeemStatus::Paid as i32, "round {round}");
    let rejected = app.deposit(&stub, &spend_b).await;
    assert_eq!(
      rejected.status,
      RedeemStatus::RejectedConflict as i32,
      "round {round}"
    );
    let proof = rejected.proof.expect("conflict carries a proof");
    assert_eq!(proof.conflict_kind, ConflictKind::SameCheque as i32);
    assert_eq!(proof.offender_account_id, payer.account_id);
    let started = Instant::now();
    IndependentVerifier::verify(&proof, &server_key)
      .unwrap_or_else(|error| panic!("round {round}: {error}"));
    let elapsed = started.elapsed();
    verifier_time += elapsed;
    verifier_max = verifier_max.max(elapsed);

    // The verifier is not vacuous: a tampered spend body is refused.
    let mut forged = proof.clone();
    forged.spend_2.as_mut().unwrap().body[0] ^= 1;
    assert!(IndependentVerifier::verify(&forged, &server_key).is_err());
    // The same spend twice is no conflict.
    let mut same = proof.clone();
    same.spend_2 = same.spend_1.clone();
    same.stub_2 = same.stub_1.clone();
    assert!(IndependentVerifier::verify(&same, &server_key).is_err());
    // The serialized proof holds no private key material.
    let secret = payer.secret_bytes();
    let bytes = proof.encode_to_vec();
    assert!(
      !bytes
        .windows(secret.len())
        .any(|window| window == secret.as_slice())
    );

    let (frozen, proofs): (bool, i64) = sqlx::query_as(
      "SELECT a.frozen, (SELECT count(*) FROM equivocation_proofs WHERE offender_account_id = a.account_id)
       FROM accounts a WHERE a.account_id = $1",
    )
    .bind(&payer.account_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(
      frozen && proofs == 1,
      "round {round}: frozen={frozen} proofs={proofs}"
    );
    assert_eq!(app.balance(&second_payee.account_id).await.1, 0);
  }
  // Payees were paid once per round and never by the rejected side.
  let (available, _) = app.balance(&second_payee.account_id).await;
  assert_eq!(available, 10_000, "rejected payee must stay at the grant");
  let average_ms = verifier_time.as_secs_f64() * 1_000.0 / rounds as f64;
  eprintln!(
    "PV-CRIP-01 same-cheque verifier: rounds={rounds}, average_ms={average_ms:.3}, max_ms={:.3}",
    verifier_max.as_secs_f64() * 1_000.0
  );
}

#[tokio::test]
async fn same_counter_deposit_proof_passes_independent_verifier() {
  let Some(app) = setup(true).await else { return };
  let (payer, first_payee, second_payee) = (Client::new(), Client::new(), Client::new());
  payer.register(&app).await;
  first_payee.register(&app).await;
  second_payee.register(&app).await;
  let (_, book) = payer.issue(&app, random_bytes(16), 2, 100, 60).await;
  let stubs = book.unwrap().stubs;
  let first = payer.spend(&stubs[0], &first_payee.account_id, 10, 7);
  let second = payer.spend(&stubs[1], &second_payee.account_id, 20, 7);

  assert_eq!(
    app.deposit(&stubs[0], &first).await.status,
    RedeemStatus::Paid as i32
  );
  let response = app.deposit(&stubs[1], &second).await;
  assert_eq!(response.status, RedeemStatus::Paid as i32);
  let proof = response
    .proof
    .expect("same-counter deposit carries a proof");
  assert_eq!(proof.conflict_kind, ConflictKind::SameCounter as i32);
  let started = Instant::now();
  IndependentVerifier::verify(&proof, &app.state.public_key()).unwrap();
  let elapsed = started.elapsed();
  eprintln!(
    "PV-CRIP-01 same-counter verifier: {:.3} ms",
    elapsed.as_secs_f64() * 1_000.0
  );
}

struct IndependentFixture {
  server_key: SigningKey,
  server_public_key: Vec<u8>,
  payer_key: SigningKey,
  payer_public_key: Vec<u8>,
  payer_account_id: Vec<u8>,
}

impl IndependentFixture {
  fn new() -> Self {
    let server_key = signing_key_from_seed(&[17; 32]).unwrap();
    let payer_key = signing_key_from_seed(&[19; 32]).unwrap();
    let server_public_key = compressed_public_key(&server_key);
    let payer_public_key = compressed_public_key(&payer_key);
    let payer_account_id = key_id(&payer_public_key);
    Self {
      server_key,
      server_public_key,
      payer_key,
      payer_public_key,
      payer_account_id,
    }
  }

  fn stub(&self, cheque_id: Vec<u8>) -> ChequeStub {
    let body = ChequeStubBody {
      cheque_id,
      payer_account_id: self.payer_account_id.clone(),
      payer_epoch: 1,
      signer_pubkey: self.payer_public_key.clone(),
      max_amount_units: 100,
      expires_at_unix: 1_900_000_060,
      server_key_id: key_id(&self.server_public_key),
      ..Default::default()
    }
    .encode_to_vec();
    ChequeStub {
      server_signature: sign(&self.server_key, &stub_seal_message(&body)),
      body,
    }
  }

  fn spend(&self, cheque_id: &[u8], counter: u64, amount_units: u64) -> ChequeSpend {
    let mut body = ChequeSpendBody {
      cheque_id: cheque_id.to_vec(),
      payee_account_id: b"independent-payee".to_vec(),
      amount_units,
      counter,
      signed_at_unix: 1,
      ..Default::default()
    };
    body.tx_id = derive_tx_id(counter, &body.encode_to_vec()).unwrap();
    let body = body.encode_to_vec();
    ChequeSpend {
      payer_signature: sign(&self.payer_key, &spend_signing_message(&body)),
      body,
    }
  }

  fn proof(
    &self,
    kind: ConflictKind,
    first: (&ChequeStub, &ChequeSpend),
    second: (&ChequeStub, &ChequeSpend),
  ) -> EquivocationProof {
    EquivocationProof {
      offender_account_id: self.payer_account_id.clone(),
      epoch: 1,
      conflict_kind: kind as i32,
      stub_1: Some(first.0.clone()),
      spend_1: Some(first.1.clone()),
      stub_2: Some(second.0.clone()),
      spend_2: Some(second.1.clone()),
    }
  }
}

#[test]
fn independent_verifier_checks_conflict_matrix_and_10000_honest_sequences() {
  let fixture = IndependentFixture::new();
  let same_cheque_a = fixture.stub(b"same-cheque".to_vec());
  let same_cheque_b = fixture.stub(b"same-cheque".to_vec());
  let different_cheque_a = fixture.stub(b"cheque-a".to_vec());
  let different_cheque_b = fixture.stub(b"cheque-b".to_vec());
  let first = fixture.spend(b"same-cheque", 10, 10);
  let second = fixture.spend(b"same-cheque", 11, 20);
  let same_cheque = fixture.proof(
    ConflictKind::SameCheque,
    (&same_cheque_a, &first),
    (&same_cheque_b, &second),
  );
  IndependentVerifier::verify(&same_cheque, &fixture.server_public_key).unwrap();

  let same_counter_a = fixture.spend(b"cheque-a", 12, 10);
  let same_counter_b = fixture.spend(b"cheque-b", 12, 20);
  let same_counter = fixture.proof(
    ConflictKind::SameCounter,
    (&different_cheque_a, &same_counter_a),
    (&different_cheque_b, &same_counter_b),
  );
  IndependentVerifier::verify(&same_counter, &fixture.server_public_key).unwrap();

  let no_conflict_a = fixture.spend(b"cheque-a", 13, 10);
  let no_conflict_b = fixture.spend(b"cheque-b", 14, 20);
  let no_conflict = fixture.proof(
    ConflictKind::SameCounter,
    (&different_cheque_a, &no_conflict_a),
    (&different_cheque_b, &no_conflict_b),
  );
  assert_eq!(
    IndependentVerifier::verify(&no_conflict, &fixture.server_public_key),
    Err("not a conflict".to_owned())
  );

  let started = Instant::now();
  let mut verifier_time = Duration::ZERO;
  for sequence in 0..10_000u64 {
    let mut first_id = sequence.to_be_bytes().to_vec();
    first_id.extend(random_bytes(8));
    let mut second_id = (sequence + 10_000).to_be_bytes().to_vec();
    second_id.extend(random_bytes(8));
    let first_stub = fixture.stub(first_id.clone());
    let second_stub = fixture.stub(second_id.clone());
    let first_counter = sequence * 2 + 1;
    let second_counter = first_counter + 1;
    let first_spend = fixture.spend(
      &first_id,
      first_counter,
      1 + u64::from(random_bytes(1)[0]) % 100,
    );
    let second_spend = fixture.spend(
      &second_id,
      second_counter,
      1 + u64::from(random_bytes(1)[0]) % 100,
    );
    let proof = fixture.proof(
      ConflictKind::SameCounter,
      (&first_stub, &first_spend),
      (&second_stub, &second_spend),
    );
    let verify_started = Instant::now();
    let result = IndependentVerifier::verify(&proof, &fixture.server_public_key);
    verifier_time += verify_started.elapsed();
    assert_eq!(
      result,
      Err("not a conflict".to_owned()),
      "honest sequence {sequence}"
    );
  }
  let elapsed = started.elapsed();
  eprintln!(
    "PV-CRIP-01 non-conflicts: sequences=10000, verifier_total_ms={:.1}, verifier_avg_us={:.2}, total_ms={:.1}, false_positives=0",
    verifier_time.as_secs_f64() * 1_000.0,
    verifier_time.as_secs_f64() * 1_000_000.0 / 10_000.0,
    elapsed.as_secs_f64() * 1_000.0
  );
}
