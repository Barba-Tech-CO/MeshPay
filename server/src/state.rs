use crate::attestation::trust_store::TrustStore;
use crate::config::app_config::AppConfig;
use crate::crypto::p256_keys::{compressed_public_key, key_id, signing_key_from_seed};
use crate::crypto::server_key_cert::{is_current, verify_cert};
use crate::http::register_limiter::RegisterLimiter;
use crate::wire::server_key_cert::ServerKeyCert;
use crate::wire::server_key_cert_body::ServerKeyCertBody;
use crate::wire::server_key_response::ServerKeyResponse;
use crate::wire::server_key_role::ServerKeyRole;
use axum::body::Bytes;
use p256::ecdsa::SigningKey;
use prost::Message;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// (built at, signed `RevocationSnapshot` bytes).
pub type CachedSnapshot = (Instant, Vec<u8>);

#[derive(Clone)]
pub struct AppState {
  pub pool: PgPool,
  pub config: Arc<AppConfig>,
  pub signing_key: SigningKey,
  pub register_limiter: Arc<RegisterLimiter>,
  pub challenge_limiter: Arc<RegisterLimiter>,
  pub control_limiter: Arc<RegisterLimiter>,
  pub snapshot_limiter: Arc<RegisterLimiter>,
  /// Encoded `ServerKeyResponse`, fixed for the process lifetime.
  pub server_key_response: Bytes,
  /// Last `RevocationSnapshot`, reused for one second.
  pub snapshot_cache: Arc<Mutex<Option<CachedSnapshot>>>,
  /// SNAPSHOT key: signs the `RevocationSnapshot` served to receivers.
  pub snapshot_key: SigningKey,
  /// Own SNAPSHOT cert, verified against the root at startup.
  pub snapshot_cert: ServerKeyCert,
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
    let (key_cert, key_cert_body) = load_cert(
      &config.server_key_cert_hex,
      &root_public_key,
      &signing_key,
      ServerKeyRole::Issuer,
    )?;
    let snapshot_seed = hex::decode(&config.server_snapshot_key_hex)
      .map_err(|_| "snapshot signing key is not hex".to_owned())?;
    let snapshot_key = signing_key_from_seed(&snapshot_seed)?;
    let (snapshot_cert, snapshot_cert_body) = load_cert(
      &config.server_snapshot_key_cert_hex,
      &root_public_key,
      &snapshot_key,
      ServerKeyRole::Snapshot,
    )?;
    if snapshot_cert_body.pubkey == key_cert_body.pubkey {
      return Err("SNAPSHOT key must differ from the ISSUER key".to_owned());
    }
    let server_key_response = ServerKeyResponse {
      public_key: key_cert_body.pubkey.clone(),
      key_id: key_cert_body.key_id.clone(),
      root_public_key: root_public_key.clone(),
      issuer_cert: key_cert.encode_to_vec(),
      snapshot_cert: snapshot_cert.encode_to_vec(),
    }
    .encode_to_vec()
    .into();
    Ok(Self {
      snapshot_limiter: Arc::new(RegisterLimiter::new()),
      server_key_response,
      snapshot_cache: Arc::new(Mutex::new(None)),
      snapshot_key,
      snapshot_cert,
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

  pub fn snapshot_public_key(&self) -> Vec<u8> {
    compressed_public_key(&self.snapshot_key)
  }
}

/// Refuses to run under a cert of another key, another role, not signed by the root or not
/// current now.
fn load_cert(
  cert_hex: &str,
  root_public_key: &[u8],
  key: &SigningKey,
  role: ServerKeyRole,
) -> Result<(ServerKeyCert, ServerKeyCertBody), String> {
  let bytes = hex::decode(cert_hex).map_err(|_| format!("{role:?} key cert is not hex"))?;
  let cert =
    ServerKeyCert::decode(bytes.as_slice()).map_err(|e| format!("server key cert: {e}"))?;
  let body = verify_cert(&cert, root_public_key, role).map_err(|e| e.to_string())?;
  if body.pubkey != compressed_public_key(key) {
    return Err(format!("{role:?} key cert is for another key"));
  }
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|e| format!("clock: {e}"))?
    .as_secs();
  if !is_current(&body, now) {
    return Err(format!("{role:?} key cert is not valid now"));
  }
  Ok((cert, body))
}
