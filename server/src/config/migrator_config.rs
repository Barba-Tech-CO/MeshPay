use crate::config::app_config::{parsed, required};
use crate::config::db_password::{ensure_hex_password, ensure_hex_url_password};

/// Admin-role settings, read only by the one-shot `meshpay_server migrate` command.
pub struct MigratorConfig {
  pub migrator_database_url: String,
  pub runtime_db_password: String,
  pub supply_cap_units: u64,
  pub initial_grant_units: u64,
}

impl MigratorConfig {
  pub fn from_env() -> Result<Self, String> {
    let limit = i64::MAX.unsigned_abs();
    let supply_cap_units: u64 = parsed("SUPPLY_CAP_UNITS")?;
    let initial_grant_units: u64 = parsed("INITIAL_GRANT_UNITS")?;
    if supply_cap_units > limit || initial_grant_units > supply_cap_units {
      return Err("SUPPLY_CAP_UNITS must fit i64 and be >= INITIAL_GRANT_UNITS".to_owned());
    }
    let migrator_database_url = required("MIGRATOR_DATABASE_URL")?;
    ensure_hex_url_password("MIGRATOR_DATABASE_URL", &migrator_database_url)?;
    let runtime_db_password = required("RUNTIME_DB_PASSWORD")?;
    ensure_hex_password("RUNTIME_DB_PASSWORD", &runtime_db_password)?;
    Ok(Self {
      migrator_database_url,
      runtime_db_password,
      supply_cap_units,
      initial_grant_units,
    })
  }
}
