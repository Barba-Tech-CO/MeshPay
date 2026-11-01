use rusqlite::Connection;

#[test]
fn encrypted_database_cannot_be_read_without_its_key() -> Result<(), Box<dyn std::error::Error>> {
  let directory = tempfile::tempdir()?;
  let database_path = directory.path().join("wallet.db");

  {
    let connection = Connection::open(&database_path)?;
    connection.pragma_update(None, "key", "meshpay-test-key")?;
    connection.execute_batch(
      "CREATE TABLE secret (value TEXT NOT NULL);
             INSERT INTO secret (value) VALUES ('encrypted');",
    )?;
  }

  let unkeyed_connection = Connection::open(&database_path)?;
  let result = unkeyed_connection.query_row("SELECT value FROM secret", [], |row| {
    row.get::<_, String>(0)
  });

  assert!(result.is_err());
  Ok(())
}
