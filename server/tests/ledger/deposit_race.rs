//! PV-LED-02 / PV-SEC-04: concurrent deposits of one cheque_id. `MESHPAY_RACE_ROUNDS`
//! defaults to 1,000 (PRD scale); override for faster local runs.
use crate::support::{Client, TestApp, random_bytes, setup};
use meshpay_server::wire::cheque_spend::ChequeSpend;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::redeem_status::RedeemStatus;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Barrier;
use tokio::task::JoinSet;

const CONCURRENCY: usize = 10;

async fn race(app: &Arc<TestApp>, stub: &ChequeStub, spends: Vec<ChequeSpend>) -> Vec<i32> {
  let mut set = JoinSet::new();
  let barrier = Arc::new(Barrier::new(CONCURRENCY));
  for (index, spend) in spends.into_iter().enumerate() {
    let (app, stub, barrier) = (app.clone(), stub.clone(), barrier.clone());
    set.spawn(async move {
      barrier.wait().await;
      (index, app.deposit(&stub, &spend).await.status)
    });
  }
  let mut statuses = vec![0; CONCURRENCY];
  while let Some(done) = set.join_next().await {
    let (index, status) = done.unwrap();
    statuses[index] = status;
  }
  statuses
}

#[tokio::test]
async fn concurrent_deposits_pay_exactly_once() {
  let Some(app) = setup(true).await else { return };
  let app = Arc::new(app);
  let rounds: usize = std::env::var("MESHPAY_RACE_ROUNDS")
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(1_000);
  let started = Instant::now();
  let payee = Client::new();
  payee.register(&app).await;
  let mut expected_credit = 0;
  for round in 0..rounds {
    let same_body = round % 2 == 0;
    let payer = Client::new();
    payer.register(&app).await;
    let (_, book) = payer.issue(&app, random_bytes(16), 1, 100, 60).await;
    let stub = book.unwrap().stubs.remove(0);
    let spends: Vec<ChequeSpend> = (0..CONCURRENCY)
      .map(|i| {
        let amount = if same_body { 40 } else { 10 + i as u64 };
        payer.spend(&stub, &payee.account_id, amount, 1)
      })
      .collect();
    let paid = RedeemStatus::Paid as i32;
    let losing = if same_body {
      RedeemStatus::AlreadyRedeemedSameBody as i32
    } else {
      RedeemStatus::RejectedConflict as i32
    };

    let statuses = race(&app, &stub, spends.clone()).await;
    let winners: Vec<usize> = (0..CONCURRENCY).filter(|i| statuses[*i] == paid).collect();
    assert_eq!(winners.len(), 1, "round {round}: statuses {statuses:?}");
    let winner = winners[0];
    for (i, status) in statuses.iter().enumerate() {
      assert!(
        i == winner || *status == losing,
        "round {round}: {statuses:?}"
      );
    }
    let amount = if same_body { 40 } else { 10 + winner as u64 };
    expected_credit += amount as i64;
    let (available, reserved) = app.balance(&payee.account_id).await;
    assert_eq!(
      available + reserved,
      10_000 + expected_credit,
      "round {round}"
    );

    // Resend of the paid body returns the previous result; replays of every request are never paid.
    assert_eq!(
      app.deposit(&stub, &spends[winner]).await.status,
      RedeemStatus::AlreadyRedeemedSameBody as i32,
      "round {round}: resend"
    );
    for spend in &spends {
      assert_ne!(
        app.deposit(&stub, spend).await.status,
        paid,
        "round {round}: replay paid"
      );
    }
    assert_eq!(
      app.balance(&payee.account_id).await.0 + app.balance(&payee.account_id).await.1,
      10_000 + expected_credit,
      "round {round}: replay credited"
    );
  }
  eprintln!(
    "PV-SEC-04 completed {rounds} rounds in {:.2?}",
    started.elapsed()
  );
}

/// PV-NET-04 lock order: deposits lock only the payer row, so A paying B while B pays A must not
/// deadlock on the two balance rows (every ledger write takes `supply` before `balances`).
#[tokio::test]
async fn opposite_direction_deposits_never_deadlock() {
  let Some(app) = setup(true).await else { return };
  let app = Arc::new(app);
  for _ in 0..5 {
    let (a, b) = (Client::new(), Client::new());
    a.register(&app).await;
    b.register(&app).await;
    let mut jobs: Vec<(ChequeStub, ChequeSpend)> = Vec::new();
    for (payer, payee) in [(&a, &b), (&b, &a)] {
      let (_, book) = payer.issue(&app, random_bytes(16), 10, 1, 600).await;
      for (counter, stub) in (1..).zip(book.expect("issue").stubs) {
        let spend = payer.spend(&stub, &payee.account_id, 1, counter);
        jobs.push((stub, spend));
      }
    }
    let barrier = Arc::new(Barrier::new(jobs.len()));
    let mut set = JoinSet::new();
    for (stub, spend) in jobs {
      let (app, barrier) = (app.clone(), barrier.clone());
      set.spawn(async move {
        barrier.wait().await;
        app.deposit(&stub, &spend).await.status
      });
    }
    while let Some(status) = set.join_next().await {
      assert_eq!(status.unwrap(), RedeemStatus::Paid as i32);
    }
    for client in [&a, &b] {
      let (available, reserved) = app.balance(&client.account_id).await;
      assert_eq!(available + reserved, 10_000);
    }
  }
}
