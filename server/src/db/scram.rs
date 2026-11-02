use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

/// Postgres SCRAM-SHA-256 verifier (RFC 5802 / 7677), so `CREATE ROLE` never carries the cleartext
/// password: `SCRAM-SHA-256$4096:<salt>$<StoredKey>:<ServerKey>`.
pub fn scram_sha256_verifier(password: &str) -> Result<String, String> {
  const ITERATIONS: u32 = 4096;
  let mut salt = [0u8; 16];
  getrandom::fill(&mut salt).map_err(|e| format!("random: {e}"))?;
  let mut salted = [0u8; 32];
  pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, ITERATIONS, &mut salted);
  let hmac = |key: &[u8], message: &[u8]| -> Result<Vec<u8>, String> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).map_err(|e| e.to_string())?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
  };
  let stored_key = Sha256::digest(hmac(&salted, b"Client Key")?);
  let server_key = hmac(&salted, b"Server Key")?;
  Ok(format!(
    "SCRAM-SHA-256${ITERATIONS}:{}${}:{}",
    STANDARD.encode(salt),
    STANDARD.encode(stored_key),
    STANDARD.encode(server_key)
  ))
}
