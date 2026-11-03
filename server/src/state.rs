use crate::attestation::trust_store::TrustStore;
use crate::config::app_config::AppConfig;
use crate::crypto::p256_keys::{compressed_public_key, key_id, signing_key_from_seed};
use crate::http::register_limiter::RegisterLimiter;
use p256::ecdsa::SigningKey;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
  pub pool: PgPool,
  pub config: Arc<AppConfig>,
  pub signing_key: SigningKey,
  pub register_limiter: Arc<RegisterLimiter>,
  pub challenge_limiter: Arc<RegisterLimiter>,
  pub trust_store: Arc<TrustStore>,
}

impl AppState {
  pub fn new(pool: PgPool, config: AppConfig) -> Result<Self, String> {
    let seed = hex::decode(&config.server_signing_key_hex)
      .map_err(|_| "server signing key is not hex".to_owned())?;
    let trust_store = TrustStore::load(
      &config.attestation_roots_pem_file,
      &config.attestation_revocation_file,
    )?;
    Ok(Self {
      pool,
      trust_store: Arc::new(trust_store),
      challenge_limiter: Arc::new(RegisterLimiter::new()),
      signing_key: signing_key_from_seed(&seed)?,
      config: Arc::new(config),
      register_limiter: Arc::new(RegisterLimiter::new()),
    })
  }

  pub fn public_key(&self) -> Vec<u8> {
    compressed_public_key(&self.signing_key)
  }

  pub fn key_id(&self) -> Vec<u8> {
    key_id(&self.public_key())
  }
}
