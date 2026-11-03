//! PV-NET-04: 1,000 distinct cheques verified and settled in under 2 s. The PRD names 2 vCPUs /
//! 2 GB hardware; this runs on the dev machine, so the measured time is printed. `MESHPAY_THROUGHPUT_STRICT=1` asserts the 2 s PRD bound.
use crate::support::{Client, TestApp, random_bytes, setup};
use meshpay_server::wire::cheque_spend::ChequeSpend;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::redeem_status::RedeemStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const DEPOSITS: usize = 1_000;
const PAYERS: usize = 100; // a chequebook holds at most 10 cheques
const CONCURRENCY: usize = 16;

#[tokio::test]
async fn thousand_deposits_settle_under_two_seconds() {
  let Some(app) = setup(true).await else { return };
  let app = Arc::new(app);
  let payee = Client::new();
  payee.register(&app).await;
  let mut jobs: Vec<(ChequeStub, ChequeSpend)> = Vec::new();
  for _ in 0..PAYERS {
    let payer = Client::new();
    payer.register(&app).await;
    let per_payer = (DEPOSITS / PAYERS) as u32;
    let (_, book) = payer.issue(&app, random_bytes(16), per_payer, 1, 600).await;
    // Honest spends: one counter per cheque (a reused counter is an equivocation, PRD 4.2).
    for (counter, stub) in (1..).zip(book.expect("issue").stubs) {
      let spend = payer.spend(&stub, &payee.account_id, 1, counter);
      jobs.push((stub, spend));
    }
  }
  assert_eq!(jobs.len(), DEPOSITS);
  let (before_available, before_reserved) = app.balance(&payee.account_id).await;

  let limit = Arc::new(Semaphore::new(CONCURRENCY));
  let started = Instant::now();
  let mut set = JoinSet::new();
  for (stub, spend) in jobs {
    let (app, limit): (Arc<TestApp>, _) = (app.clone(), limit.clone());
    set.spawn(async move {
      let _permit = limit.acquire().await.unwrap();
      app.deposit(&stub, &spend).await.status
    });
  }
  while let Some(status) = set.join_next().await {
    assert_eq!(status.unwrap(), RedeemStatus::Paid as i32);
  }
  let elapsed = started.elapsed();
  eprintln!("PV-NET-04: {DEPOSITS} deposits in {elapsed:?} (concurrency {CONCURRENCY})");

  let (available, reserved) = app.balance(&payee.account_id).await;
  assert_eq!(
    available + reserved - before_available - before_reserved,
    DEPOSITS as i64
  );
  // PRD bound (2 s) is enforced with MESHPAY_THROUGHPUT_STRICT=1 (release build); the default is a
  // regression guard only, since debug builds and shared CI hosts are slower.
  let bound = if std::env::var("MESHPAY_THROUGHPUT_STRICT").is_ok() {
    Duration::from_secs(2)
  } else {
    Duration::from_secs(30)
  };
  assert!(
    elapsed < bound,
    "{DEPOSITS} deposits took {elapsed:?}, bound {bound:?}"
  );
}
