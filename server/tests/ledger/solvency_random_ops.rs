//! PV-LED-01: random operations never break solvency (balances + reserves <= supply).
//! Env: `MESHPAY_LEDGER_OPS` (default 10000, the PRD figure), `MESHPAY_LEDGER_SEED` (replay).
use crate::random_source::RandomSource;
use crate::support::{Client, random_bytes, setup};
use axum::http::StatusCode;
use meshpay_server::wire::cheque_spend::ChequeSpend;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::redeem_status::RedeemStatus;
use std::sync::Arc;
use tokio::task::JoinSet;

struct IssuedCheque {
  payer: usize,
  stub: ChequeStub,
  max: u64,
  short_lived: bool,
  book_id: Vec<u8>,
  paid_spend: Option<ChequeSpend>,
}

async fn assert_solvency(pool: &sqlx::PgPool, context: &str) {
  let (available, reserves, supply, negative): (i64, i64, i64, i64) = sqlx::query_as(
    "SELECT (SELECT coalesce(sum(available), 0)::bigint FROM balances),
            (SELECT coalesce(sum(reserved), 0)::bigint FROM balances),
            (SELECT granted FROM supply),
            (SELECT count(*) FROM balances WHERE available < 0 OR reserved < 0)",
  )
  .fetch_one(pool)
  .await
  .unwrap();
  assert!(
    available + reserves <= supply && negative == 0,
    "{context}: available={available} reserves={reserves} supply={supply} negative={negative}"
  );
}

#[tokio::test]
async fn random_operations_keep_the_ledger_solvent() {
  let Some(app) = setup(true).await else { return };
  let app = Arc::new(app);
  let ops: usize = std::env::var("MESHPAY_LEDGER_OPS")
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(10_000);
  let (mut rng, seed) = RandomSource::from_env_or_os("MESHPAY_LEDGER_SEED");
  eprintln!("PV-LED-01 seed={seed} ops={ops} (replay with MESHPAY_LEDGER_SEED={seed})");

  let mut accounts: Vec<Client> = Vec::new();
  let mut counters: Vec<u64> = Vec::new();
  let mut cheques: Vec<IssuedCheque> = Vec::new();
  for _ in 0..5 {
    let client = Client::new();
    client.register(&app).await;
    accounts.push(client);
    counters.push(0);
  }
  let mut last_expiry = 0;
  let status_ok = |status: i32, allowed: &[RedeemStatus], what: &str| {
    assert!(
      allowed.iter().any(|a| *a as i32 == status),
      "seed={seed}: {what}: unexpected status {status}"
    );
  };

  for op in 0..ops {
    if op > 0 && op % 100 == 0 {
      let (available, reserves, supply, negative): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT coalesce(sum(available), 0)::bigint FROM balances),
                (SELECT coalesce(sum(reserved), 0)::bigint FROM balances),
                (SELECT granted FROM supply),
                (SELECT count(*) FROM balances WHERE available < 0 OR reserved < 0)",
      )
      .fetch_one(&app.pool)
      .await
      .unwrap();
      assert!(
        available + reserves <= supply && negative == 0,
        "seed={seed} batch={} available={available} reserves={reserves} supply={supply} negative={negative}",
        op / 100 - 1
      );
      let ids: Vec<Vec<u8>> = accounts.iter().map(|c| c.account_id.clone()).collect();
      let total: i64 = sqlx::query_scalar(
        "SELECT coalesce(sum(available + reserved), 0)::bigint FROM balances WHERE account_id = ANY($1)",
      )
      .bind(&ids)
      .fetch_one(&app.pool)
      .await
      .unwrap();
      assert_eq!(
        total,
        10_000 * ids.len() as i64,
        "seed={seed} op={op}: money not conserved"
      );
    }

    let who = rng.below(accounts.len() as u64) as usize;
    let roll = rng.below(100);
    match roll {
      0..10 => {
        if accounts.len() < 200 {
          let client = Client::new();
          assert_eq!(
            client.register(&app).await.0,
            StatusCode::OK,
            "seed={seed} op={op}"
          );
          accounts.push(client);
          counters.push(0);
        }
      }
      10..40 | 95..100 => {
        let short_lived = rng.below(5) == 0;
        let (count, max) = (1 + rng.below(3) as u32, 1 + rng.below(200));
        let ttl = if short_lived { 1 } else { 3_600 };
        let (status, book) = accounts[who]
          .issue(&app, random_bytes(16), count, max, ttl)
          .await;
        match book {
          Some(book) => {
            let book_id = book.chequebook_id;
            for stub in book.stubs {
              cheques.push(IssuedCheque {
                payer: who,
                stub,
                max,
                short_lived,
                book_id: book_id.clone(),
                paid_spend: None,
              });
            }
          }
          None => assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::UNPROCESSABLE_ENTITY,
            "seed={seed} op={op}: issue status {status}"
          ),
        }
      }
      40..75 => {
        let open: Vec<usize> = (0..cheques.len())
          .filter(|i| cheques[*i].paid_spend.is_none())
          .collect();
        if !open.is_empty() {
          // Settle a small random batch concurrently so the workload exercises concurrent deposits.
          let count = (1 + rng.below(4) as usize).min(open.len());
          let mut selected = Vec::with_capacity(count);
          while selected.len() < count {
            let index = open[rng.below(open.len() as u64) as usize];
            if !selected.contains(&index) {
              selected.push(index);
            }
          }
          let mut jobs = JoinSet::new();
          for index in selected {
            let payer = cheques[index].payer;
            let payee =
              (payer + 1 + rng.below(accounts.len() as u64 - 1) as usize) % accounts.len();
            let amount = 1 + rng.below(cheques[index].max);
            counters[payer] += 1;
            let spend = accounts[payer].spend(
              &cheques[index].stub,
              &accounts[payee].account_id,
              amount,
              counters[payer],
            );
            let app = app.clone();
            let stub = cheques[index].stub.clone();
            jobs.spawn(async move {
              let response = app.deposit(&stub, &spend).await;
              (index, spend, response.status, response.detail)
            });
          }
          while let Some(result) = jobs.join_next().await {
            let (index, spend, status, detail) = result.unwrap();
            let allowed: &[RedeemStatus] = if cheques[index].short_lived {
              &[RedeemStatus::Paid, RedeemStatus::RejectedExpired]
            } else {
              &[RedeemStatus::Paid]
            };
            status_ok(status, allowed, &format!("op={op} pay: {detail}"));
            if status == RedeemStatus::Paid as i32 {
              cheques[index].paid_spend = Some(spend);
            }
          }
          assert_solvency(
            &app.pool,
            &format!("seed={seed} op={op} after concurrent batch"),
          )
          .await;
        }
      }
      75..85 => {
        let open: Vec<usize> = (0..cheques.len())
          .filter(|i| cheques[*i].paid_spend.is_none())
          .collect();
        if !open.is_empty() && accounts.len() >= 3 {
          let index = open[rng.below(open.len() as u64) as usize];
          let payer = cheques[index].payer;
          let payee_a = (payer + 1) % accounts.len();
          let payee_b = (payer + 2) % accounts.len();
          counters[payer] += 1;
          let stub = cheques[index].stub.clone();
          let spend_a =
            accounts[payer].spend(&stub, &accounts[payee_a].account_id, 1, counters[payer]);
          let spend_b =
            accounts[payer].spend(&stub, &accounts[payee_b].account_id, 1, counters[payer]);
          let first = app.deposit(&stub, &spend_a).await;
          let allowed = [RedeemStatus::Paid, RedeemStatus::RejectedExpired];
          status_ok(first.status, &allowed, &format!("op={op} conflict first"));
          if first.status == RedeemStatus::Paid as i32 {
            cheques[index].paid_spend = Some(spend_a);
            let second = app.deposit(&stub, &spend_b).await;
            status_ok(
              second.status,
              &[RedeemStatus::RejectedConflict],
              &format!("op={op} conflict"),
            );
          }
        }
      }
      _ => {
        let paid: Vec<usize> = (0..cheques.len())
          .filter(|i| cheques[*i].paid_spend.is_some())
          .collect();
        if !paid.is_empty() {
          let cheque = &cheques[paid[rng.below(paid.len() as u64) as usize]];
          let replay = app
            .deposit(&cheque.stub, cheque.paid_spend.as_ref().unwrap())
            .await;
          let allowed = [RedeemStatus::AlreadyRedeemedSameBody];
          status_ok(replay.status, &allowed, &format!("op={op} replay"));
        }
      }
    }

    // Advance the simulated clock by expiring one short-lived chequebook in storage, then
    // issue again for its payer to exercise release and reserve recovery without wall-clock sleep.
    if op - last_expiry >= ops.max(5) / 5 {
      last_expiry = op;
      let candidates: Vec<usize> = cheques
        .iter()
        .enumerate()
        .filter(|(_, cheque)| cheque.short_lived && cheque.paid_spend.is_none())
        .map(|(index, _)| index)
        .collect();
      if !candidates.is_empty() {
        let index = candidates[rng.below(candidates.len() as u64) as usize];
        let payer = cheques[index].payer;
        let book_id = cheques[index].book_id.clone();
        // Test-only clock fixture: bypass append-only guards only for this transaction's expiry update.
        let mut tx = app.owner_pool.begin().await.unwrap();
        sqlx::query("SET LOCAL session_replication_role = 'replica'")
          .execute(&mut *tx)
          .await
          .unwrap();
        sqlx::query("UPDATE chequebooks SET expires_at_unix = 0 WHERE chequebook_id = $1")
          .bind(&book_id)
          .execute(&mut *tx)
          .await
          .unwrap();
        sqlx::query("UPDATE cheques SET expires_at_unix = 0 WHERE chequebook_id = $1")
          .bind(book_id)
          .execute(&mut *tx)
          .await
          .unwrap();
        tx.commit().await.unwrap();
        let _ = accounts[payer]
          .issue(&app, random_bytes(16), 1, 1, 60)
          .await;
      }
    }
    assert_solvency(&app.pool, &format!("seed={seed} op={op}")).await;
  }
  assert_solvency(&app.pool, &format!("seed={seed} final")).await;
  // PV-LED-01 end state: supply cap holds, audit chain recomputes, nothing negative.
  let (supply_ok, chain_ok, negative): (bool, bool, i64) = sqlx::query_as(
    "SELECT (SELECT circulating <= granted AND granted <= cap FROM supply),
            audit_chain_valid(),
            (SELECT count(*) FROM balances WHERE available < 0 OR reserved < 0)",
  )
  .fetch_one(&app.pool)
  .await
  .unwrap();
  assert!(
    supply_ok && chain_ok && negative == 0,
    "seed={seed}: supply_ok={supply_ok} audit_chain_valid={chain_ok} negative={negative}"
  );
}
