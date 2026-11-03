use crate::db::scram::scram_sha256_verifier;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
  PgPoolOptions::new()
    .max_connections(10)
    .connect(database_url)
    .await
}

/// Admin-side setup run by `meshpay_server migrate` (and the tests), with the Postgres superuser:
/// 1. creates the non-superuser schema owner `meshpay_owner` and the runtime role `meshpay_app`
///    (password stored as a SCRAM-SHA-256 verifier, never cleartext);
/// 2. applies the migrations as `meshpay_owner`, so every table and SECURITY DEFINER function is
///    owned by a non-superuser. The supply row is created inside migration 0001 from the session
///    settings below, atomically with the schema; a missing setting fails the migration.
pub async fn migrate(
  admin_url: &str,
  supply_cap_units: u64,
  initial_grant_units: u64,
  runtime_db_password: &str,
) -> Result<(), sqlx::Error> {
  let admin = connect(admin_url).await?;
  prepare_roles(&admin, runtime_db_password).await?;
  admin.close().await;
  apply_schema(admin_url, supply_cap_units, initial_grant_units).await
}

/// Step 2 of `migrate` on the database of `admin_url`; the roles must already exist.
pub async fn apply_schema(
  admin_url: &str,
  supply_cap_units: u64,
  initial_grant_units: u64,
) -> Result<(), sqlx::Error> {
  let admin = connect(admin_url).await?;
  sqlx::query("ALTER SCHEMA public OWNER TO meshpay_owner")
    .execute(&admin)
    .await?;
  admin.close().await;
  let cap = supply_cap_units.to_string();
  let grant = initial_grant_units.to_string();
  let owner = PgPoolOptions::new()
    .max_connections(2)
    .after_connect(move |connection, _| {
      let (cap, grant) = (cap.clone(), grant.clone());
      Box::pin(async move {
        sqlx::query("SET ROLE meshpay_owner").execute(&mut *connection).await?;
        sqlx::query("SELECT set_config('meshpay.supply_cap', $1, false), set_config('meshpay.initial_grant', $2, false)")
          .bind(cap)
          .bind(grant)
          .execute(&mut *connection)
          .await?;
        Ok(())
      })
    })
    .connect(admin_url)
    .await?;
  sqlx::migrate!("./migrations").run(&owner).await?;
  owner.close().await;
  Ok(())
}

async fn prepare_roles(admin: &PgPool, runtime_db_password: &str) -> Result<(), sqlx::Error> {
  let verifier = scram_sha256_verifier(runtime_db_password).map_err(sqlx::Error::Protocol)?;
  let mut tx = admin.begin().await?;
  sqlx::query("SELECT pg_advisory_xact_lock(7260001)")
    .execute(&mut *tx)
    .await?;
  for (role, login) in [("meshpay_owner", false), ("meshpay_app", true)] {
    let exists: bool =
      sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = $1)")
        .bind(role)
        .fetch_one(&mut *tx)
        .await?;
    // format() with %I / %L quotes identifier and verifier server-side.
    let statement: String = sqlx::query_scalar(
      "SELECT format('%s ROLE %I %s NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS %s',
         $1::text, $2::text, CASE WHEN $3 THEN 'LOGIN' ELSE 'NOLOGIN' END,
         CASE WHEN $3 THEN format('PASSWORD %L', $4::text) ELSE '' END)",
    )
    .bind(if exists { "ALTER" } else { "CREATE" })
    .bind(role)
    .bind(login)
    .bind(&verifier)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(AssertSqlSafe(statement))
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}
