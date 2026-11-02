use crate::ledger::balance;
use sqlx::PgConnection;

/// Cheques released per call; the rest wait for the account's next issue.
const BATCH_SIZE: i64 = 100;

/// Returns the reserve of the account's expired, never redeemed cheques (PRD 3.3 step 5).
/// The caller holds the account row lock, so a release cannot race a redemption, and appends
/// a `CHEQUE_RELEASED` audit entry per returned id AFTER its own ledger postings (supply row
/// before audit head in every transaction, so transactions cannot deadlock).
pub async fn release_expired(
  connection: &mut PgConnection,
  account_id: &[u8],
  now_unix: i64,
) -> Result<Vec<Vec<u8>>, sqlx::Error> {
  let due: Vec<(Vec<u8>, i64)> = sqlx::query_as(
    "SELECT c.cheque_id, c.max_amount_units FROM cheques c
     WHERE c.account_id = $1 AND c.expires_at_unix < $2
       AND NOT EXISTS (SELECT 1 FROM redemptions r WHERE r.cheque_id = c.cheque_id)
       AND NOT EXISTS (SELECT 1 FROM cheque_releases l WHERE l.cheque_id = c.cheque_id)
     ORDER BY c.cheque_id LIMIT $3",
  )
  .bind(account_id)
  .bind(now_unix)
  .bind(BATCH_SIZE)
  .fetch_all(&mut *connection)
  .await?;
  for (cheque_id, units) in &due {
    sqlx::query("INSERT INTO cheque_releases (cheque_id, released_units) VALUES ($1, $2)")
      .bind(cheque_id)
      .bind(units)
      .execute(&mut *connection)
      .await?;
    balance::post_release(connection, cheque_id).await?;
  }
  Ok(due.into_iter().map(|(cheque_id, _)| cheque_id).collect())
}
