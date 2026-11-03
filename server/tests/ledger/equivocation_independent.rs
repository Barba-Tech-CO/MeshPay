//! PV-LED-02 / PV-CRIP-01: double deposits of one cheque yield a proof an independent verifier
//! accepts. Rounds: env `MESHPAY_EQUIVOCATION_ROUNDS` (default 100; the PRD figure is 1000, set via the env var).
use crate::independent_verifier::IndependentVerifier;
use crate::support::{Client, random_bytes, setup};
use meshpay_server::wire::conflict_kind::ConflictKind;
use meshpay_server::wire::redeem_status::RedeemStatus;
use prost::Message;

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
    IndependentVerifier::verify(&proof, &server_key)
      .unwrap_or_else(|error| panic!("round {round}: {error}"));

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
}
