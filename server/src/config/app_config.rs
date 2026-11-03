use crate::config::db_password::ensure_hex_url_password;
use std::env;

/// Everything comes from the environment; no defaults, so a missing value fails at startup.
pub struct AppConfig {
  /// Runtime role connection (least privilege, no DDL).
  pub database_url: String,
  pub listen_addr: String,
  /// Required `true` to accept software-attested accounts (PV-SEC-01).
  pub lab_mode: bool,
  /// P-256 seed of the lab server key, 32 bytes hex. Secret; read from a file (Docker secret).
  pub server_signing_key_hex: String,
  /// `f`: max value per cheque.
  pub cap_cheque_units: u64,
  /// `R`: max reserve per chequebook and max pending reserve per wallet.
  pub cap_reserve_units: u64,
  /// `N_max`: cheques per chequebook (at most 10, also enforced by the database).
  pub cheque_max_count: u32,
  /// `E`: max chequebook validity.
  pub cheque_ttl_max_seconds: u64,
  /// Accepted clock skew of a signed balance read, in seconds.
  pub balance_auth_window_seconds: u64,
  pub request_timeout_seconds: u64,
  pub max_concurrent_requests: usize,
  /// Registrations per minute per process. Registration grants free units: lab faucet only.
  pub register_limit_per_minute: u32,
  /// Challenge requests per minute per process (public endpoint, separate from registration).
  pub challenge_limit_per_minute: u32,
  /// PEM file with the trusted attestation roots (operator supplied; Google hardware roots).
  pub attestation_roots_pem_file: String,
  /// One revoked certificate serial (hex) per line; operator supplied.
  pub attestation_revocation_file: String,
  /// Registration challenge lifetime, 1..=60 seconds.
  pub attestation_challenge_ttl_seconds: u64,
  /// PRD 4.5: only StrongBox issues chequebooks unless the operator explicitly allows TEE.
  pub allow_tee_issuance: bool,
}

impl AppConfig {
  pub fn from_env() -> Result<Self, String> {
    let key_file = required("SERVER_SIGNING_KEY_FILE")?;
    let key = std::fs::read_to_string(&key_file)
      .map_err(|e| format!("cannot read SERVER_SIGNING_KEY_FILE: {e}"))?;
    let config = Self {
      database_url: required("DATABASE_URL")?,
      listen_addr: required("LISTEN_ADDR")?,
      lab_mode: parsed("LAB_MODE")?,
      server_signing_key_hex: key.trim().to_owned(),
      cap_cheque_units: parsed("CAP_CHEQUE_UNITS")?,
      cap_reserve_units: parsed("CAP_RESERVE_UNITS")?,
      cheque_max_count: parsed("CHEQUE_MAX_COUNT")?,
      cheque_ttl_max_seconds: parsed("CHEQUE_TTL_MAX_SECONDS")?,
      balance_auth_window_seconds: parsed("BALANCE_AUTH_WINDOW_SECONDS")?,
      request_timeout_seconds: parsed("REQUEST_TIMEOUT_SECONDS")?,
      max_concurrent_requests: parsed("MAX_CONCURRENT_REQUESTS")?,
      register_limit_per_minute: parsed("REGISTER_LIMIT_PER_MINUTE")?,
      challenge_limit_per_minute: parsed("CHALLENGE_LIMIT_PER_MINUTE")?,
      attestation_roots_pem_file: required("ATTESTATION_ROOTS_PEM_FILE")?,
      attestation_revocation_file: required("ATTESTATION_REVOCATION_FILE")?,
      attestation_challenge_ttl_seconds: parsed("ATTESTATION_CHALLENGE_TTL_SECONDS")?,
      allow_tee_issuance: parsed("ALLOW_TEE_ISSUANCE")?,
    };
    config.validate()?;
    ensure_hex_url_password("DATABASE_URL", &config.database_url)?;
    Ok(config)
  }

  /// Rejects values the database or the PRD caps cannot represent.
  pub fn validate(&self) -> Result<(), String> {
    let limit = i64::MAX.unsigned_abs();
    let amounts = [
      ("CAP_CHEQUE_UNITS", self.cap_cheque_units),
      ("CAP_RESERVE_UNITS", self.cap_reserve_units),
      ("CHEQUE_TTL_MAX_SECONDS", self.cheque_ttl_max_seconds),
      (
        "BALANCE_AUTH_WINDOW_SECONDS",
        self.balance_auth_window_seconds,
      ),
    ];
    if let Some((name, _)) = amounts.iter().find(|(_, value)| *value > limit) {
      return Err(format!("{name} exceeds i64::MAX"));
    }
    if !(1..=10).contains(&self.cheque_max_count) {
      return Err("CHEQUE_MAX_COUNT must be 1..=10".to_owned());
    }
    if self.request_timeout_seconds == 0 || self.max_concurrent_requests == 0 {
      return Err(
        "REQUEST_TIMEOUT_SECONDS and MAX_CONCURRENT_REQUESTS must be positive".to_owned(),
      );
    }
    if !(1..=60).contains(&self.attestation_challenge_ttl_seconds) {
      return Err("ATTESTATION_CHALLENGE_TTL_SECONDS must be 1..=60".to_owned());
    }
    Ok(())
  }
}

pub fn required(name: &str) -> Result<String, String> {
  env::var(name).map_err(|_| format!("missing environment variable {name}"))
}

pub fn parsed<T: std::str::FromStr>(name: &str) -> Result<T, String> {
  required(name)?
    .parse()
    .map_err(|_| format!("invalid value for {name}"))
}
