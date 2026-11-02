use sqlx::PgConnection;

/// Appends to the hash-chained audit log; the database trigger fills `prev_hash` and `hash`.
pub async fn append(
  connection: &mut PgConnection,
  event: &str,
  ref_id: &[u8],
) -> Result<(), sqlx::Error> {
  sqlx::query("INSERT INTO audit_log (event, ref_id) VALUES ($1, $2)")
    .bind(event)
    .bind(ref_id)
    .execute(connection)
    .await?;
  Ok(())
}
