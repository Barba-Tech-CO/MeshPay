use crate::error::ApiError;
use sqlx::{PgConnection, PgPool};

/// Reserve for a stored chequebook. The amounts come from the database row, not the caller.
pub async fn post_reserve(
  connection: &mut PgConnection,
  chequebook_id: &[u8],
) -> Result<(), sqlx::Error> {
  sqlx::query("SELECT post_reserve($1)")
    .bind(chequebook_id)
    .execute(connection)
    .await?;
  Ok(())
}

/// Settles the payer's reserve and credits the payee for a stored redemption.
pub async fn post_redeem(
  connection: &mut PgConnection,
  cheque_id: &[u8],
) -> Result<(), sqlx::Error> {
  sqlx::query("SELECT post_redeem($1)")
    .bind(cheque_id)
    .execute(connection)
    .await?;
  Ok(())
}

/// Returns the reserve of a stored, expired and unredeemed cheque.
pub async fn post_release(
  connection: &mut PgConnection,
  cheque_id: &[u8],
) -> Result<(), sqlx::Error> {
  sqlx::query("SELECT post_release($1)")
    .bind(cheque_id)
    .execute(connection)
    .await?;
  Ok(())
}

pub struct AccountBalance {
  pub available: i64,
  pub reserved: i64,
  pub frozen: bool,
}

pub async fn read_balance(pool: &PgPool, account_id: &[u8]) -> Result<AccountBalance, ApiError> {
  let row: Option<(i64, i64, bool)> = sqlx::query_as(
    "SELECT b.available, b.reserved, a.frozen FROM balances b
     JOIN accounts a USING (account_id) WHERE b.account_id = $1",
  )
  .bind(account_id)
  .fetch_optional(pool)
  .await?;
  let (available, reserved, frozen) =
    row.ok_or(ApiError::NotFound("unknown account".to_owned()))?;
  Ok(AccountBalance {
    available,
    reserved,
    frozen,
  })
}

/// Reserved units of one account, inside the caller's transaction.
pub async fn read_pending(
  connection: &mut PgConnection,
  account_id: &[u8],
) -> Result<i64, sqlx::Error> {
  sqlx::query_scalar("SELECT reserved FROM balances WHERE account_id = $1")
    .bind(account_id)
    .fetch_one(connection)
    .await
}
