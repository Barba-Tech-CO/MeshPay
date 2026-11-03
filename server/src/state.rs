use crate::attestation::trust_store::TrustStore;
use crate::config::app_config::AppConfig;
use crate::crypto::p256_keys::{compressed_public_key, key_id, signing_key_from_seed};
use crate::crypto::server_key_cert::verify_cert;
use crate::http::register_limiter::RegisterLimiter;
use crate::wire::server_key_cert::ServerKeyCert;
use crate::wire::server_key_cert_body::ServerKeyCertBody;
use crate::wire::server_key_role::ServerKeyRole;
use p256::ecdsa::SigningKey;
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
  pub pool: PgPool,
  pub config: Arc<AppConfig>,
  pub signing_key: SigningKey,
  pub register_limiter: Arc<RegisterLimiter>,
  pub challenge_limiter: Arc<RegisterLimiter>,
  pub control_limiter: Arc<RegisterLimiter>,
  pub trust_store: Arc<TrustStore>,
  /// Offline root public key (PRD 7.4); verifies every `ServerKeyCert`.
  pub root_public_key: Vec<u8>,
  /// Own ISSUER cert; its key id goes into every stub.
  pub key_cert: ServerKeyCert,
  /// `key_cert` decoded and verified once at startup; requests only check `is_current`.
  pub key_cert_body: ServerKeyCertBody,
}

impl AppState {
  pub fn new(pool: PgPool, config: AppConfig) -> Result<Self, String> {
    let seed = hex::decode(&config.server_signing_key_hex)
      .map_err(|_| "server signing key is not hex".to_owned())?;
    let trust_store = TrustStore::load(
      &config.attestation_roots_pem_file,
      &config.attestation_revocation_file,
    )?;
    let signing_key = signing_key_from_seed(&seed)?;
    let root_public_key = hex::decode(&config.server_root_public_key_hex)
      .map_err(|_| "server root public key is not hex".to_owned())?;
    let cert_bytes = hex::decode(&config.server_key_cert_hex)
      .map_err(|_| "server key cert is not hex".to_owned())?;
    let key_cert =
      ServerKeyCert::decode(cert_bytes.as_slice()).map_err(|e| format!("server key cert: {e}"))?;
    // Refuse to run under a cert of another key, another role or not signed by the root.
    let key_cert_body =
      verify_cert(&key_cert, &root_public_key, ServerKeyRole::Issuer).map_err(|e| e.to_string())?;
    if key_cert_body.pubkey != compressed_public_key(&signing_key) {
      return Err("server key cert is for another key".to_owned());
    }
    Ok(Self {
      pool,
      root_public_key,
      key_cert,
      key_cert_body,
      control_limiter: Arc::new(RegisterLimiter::new()),
      trust_store: Arc::new(trust_store),
      challenge_limiter: Arc::new(RegisterLimiter::new()),
      signing_key,
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
