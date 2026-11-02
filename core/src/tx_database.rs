use rusqlite::Connection;
use std::path::Path;
use zeroize::Zeroizing;

/// Opens (creating if needed) a SQLCipher database keyed with a raw 32-byte key
/// (no passphrase KDF), in WAL mode with `synchronous = FULL`, and runs `schema`.
pub fn open_database(path: &Path, key: &[u8], schema: &str) -> Result<Connection, String> {
  if key.len() != 32 {
    return Err("database key must be 32 bytes".to_owned());
  }
  let hex_key: Zeroizing<String> =
    Zeroizing::new(key.iter().map(|byte| format!("{byte:02x}")).collect());
  let connection = Connection::open(path).map_err(sql_error)?;
  connection
    .pragma_update(None, "key", &*Zeroizing::new(format!("x'{}'", *hex_key)))
    .map_err(sql_error)?;
  connection
    .pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))
    .map_err(sql_error)?;
  connection
    .pragma_update(None, "synchronous", "FULL")
    .map_err(sql_error)?;
  connection.execute_batch(schema).map_err(sql_error)?;
  Ok(connection)
}

pub fn sql_error(error: rusqlite::Error) -> String {
  format!("database: {error}")
}

/// SQLite integers are signed 64-bit; reject what does not fit instead of wrapping.
pub fn to_sql_integer(value: u64) -> Result<i64, String> {
  i64::try_from(value).map_err(|_| "value exceeds i64".to_owned())
}
