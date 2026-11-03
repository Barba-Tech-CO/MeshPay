use meshpay_server::config::app_config::AppConfig;
use meshpay_server::config::migrator_config::MigratorConfig;
use meshpay_server::db::pool;
use meshpay_server::http::router::router;
use meshpay_server::state::AppState;

#[tokio::main]
async fn main() -> Result<(), String> {
  tracing_subscriber::fmt().init();
  if std::env::args().nth(1).as_deref() == Some("migrate") {
    return run_migrate().await;
  }
  #[cfg(feature = "lab")]
  if std::env::args().nth(1).as_deref() == Some("lab-key-cert") {
    let args: Vec<String> = std::env::args().skip(2).collect();
    return meshpay_server::lab::key_cert::print_lab_key_cert(&args);
  }
  let config = AppConfig::from_env()?;
  let db = pool::connect(&config.database_url)
    .await
    .map_err(|e| e.to_string())?;
  let listen_addr = config.listen_addr.clone();
  let state = AppState::new(db, config)?;
  let listener = tokio::net::TcpListener::bind(&listen_addr)
    .await
    .map_err(|e| e.to_string())?;
  tracing::info!(%listen_addr, "meshpay_server listening");
  axum::serve(listener, router(state))
    .await
    .map_err(|e| e.to_string())
}

/// One-shot owner-role job: roles, schema, grants, supply row. The server never holds these credentials.
async fn run_migrate() -> Result<(), String> {
  let config = MigratorConfig::from_env()?;
  pool::migrate(
    &config.migrator_database_url,
    config.supply_cap_units,
    config.initial_grant_units,
    &config.runtime_db_password,
  )
  .await
  .map_err(|e| e.to_string())?;
  tracing::info!("migrations applied");
  Ok(())
}
