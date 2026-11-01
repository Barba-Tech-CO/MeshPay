use flutter_rust_bridge::frb;
use p256::ecdsa::{
  Signature, SigningKey,
  signature::{Signer, Verifier},
};
use rusqlite::Connection;
use snow::Builder;
use std::fs::{self, OpenOptions};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[frb(sync)]
pub fn core_version() -> String {
  env!("CARGO_PKG_VERSION").to_owned()
}

#[frb(sync)]
pub fn ping(n: u32) -> u32 {
  n + 1
}

pub fn echo(data: Vec<u8>) -> Vec<u8> {
  data
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreSelfcheck {
  pub sqlcipher_opens_with_key: bool,
  pub sqlcipher_rejects_without_key: bool,
  pub noise_xk_handshake_ok: bool,
  pub p256_sign_verify_ok: bool,
  pub rust_version: String,
  pub elapsed_micros: u64,
}

pub fn core_selfcheck(storage_dir: String) -> CoreSelfcheck {
  let started = Instant::now();
  let rust_version = rustc_version_runtime::version().to_string();
  let database_path = create_selfcheck_database_path(Path::new(&storage_dir));
  let checks = catch_unwind(AssertUnwindSafe(|| {
    let (sqlcipher_opens_with_key, sqlcipher_rejects_without_key) = database_path
      .as_deref()
      .map(run_sqlcipher_selfcheck)
      .unwrap_or((false, false));

    (
      sqlcipher_opens_with_key,
      sqlcipher_rejects_without_key,
      run_noise_xk_selfcheck(),
      run_p256_selfcheck(),
    )
  }))
  .unwrap_or((false, false, false, false));
  let cleanup_succeeded = database_path
    .as_deref()
    .map(remove_selfcheck_database)
    .unwrap_or(false);

  CoreSelfcheck {
    sqlcipher_opens_with_key: checks.0 && cleanup_succeeded,
    sqlcipher_rejects_without_key: checks.1 && cleanup_succeeded,
    noise_xk_handshake_ok: checks.2,
    p256_sign_verify_ok: checks.3,
    rust_version,
    elapsed_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
  }
}

fn create_selfcheck_database_path(storage_dir: &Path) -> Option<PathBuf> {
  fs::create_dir_all(storage_dir).ok()?;

  let timestamp = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .ok()?
    .as_nanos();
  for sequence in 0..u32::MAX {
    let path = storage_dir.join(format!(
      "meshpay-selfcheck-{}-{timestamp}-{sequence}.sqlite",
      std::process::id()
    ));
    match OpenOptions::new().write(true).create_new(true).open(&path) {
      Ok(_) => return Some(path),
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
      Err(_) => return None,
    }
  }

  None
}

fn run_sqlcipher_selfcheck(database_path: &Path) -> (bool, bool) {
  const DATABASE_KEY: &str = "meshpay-core-selfcheck-key";

  let initial_open_succeeded = (|| {
    let connection = Connection::open(database_path).ok()?;
    connection.pragma_update(None, "key", DATABASE_KEY).ok()?;
    connection
      .execute_batch("CREATE TABLE selfcheck (value TEXT NOT NULL); INSERT INTO selfcheck (value) VALUES ('ok');")
      .ok()?;
    read_selfcheck_value(&connection).then_some(())
  })()
  .is_some();
  if !initial_open_succeeded {
    return (false, false);
  }

  let no_key_was_rejected = match Connection::open(database_path) {
    Ok(connection) => !read_selfcheck_value(&connection),
    Err(_) => true,
  };
  let keyed_reopen_succeeded = (|| {
    let connection = Connection::open(database_path).ok()?;
    connection.pragma_update(None, "key", DATABASE_KEY).ok()?;
    read_selfcheck_value(&connection).then_some(())
  })()
  .is_some();

  (keyed_reopen_succeeded, no_key_was_rejected)
}

fn read_selfcheck_value(connection: &Connection) -> bool {
  connection
    .query_row("SELECT value FROM selfcheck", [], |row| {
      row.get::<_, String>(0)
    })
    .is_ok_and(|value| value == "ok")
}

fn remove_selfcheck_database(database_path: &Path) -> bool {
  [
    database_path.as_os_str().to_os_string(),
    append_path_suffix(database_path, "-journal"),
    append_path_suffix(database_path, "-wal"),
    append_path_suffix(database_path, "-shm"),
  ]
  .into_iter()
  .all(|path| match fs::remove_file(path) {
    Ok(()) => true,
    Err(error) => error.kind() == io::ErrorKind::NotFound,
  })
}

fn append_path_suffix(path: &Path, suffix: &str) -> std::ffi::OsString {
  let mut path = path.as_os_str().to_os_string();
  path.push(suffix);
  path
}

fn run_noise_xk_selfcheck() -> bool {
  (|| {
    let params = "Noise_XK_25519_ChaChaPoly_BLAKE2s"
      .parse::<snow::params::NoiseParams>()
      .ok()?;
    let responder_builder = Builder::new(params.clone());
    let responder_keys = responder_builder.generate_keypair().ok()?;
    let initiator_builder = Builder::new(params.clone());
    let initiator_keys = initiator_builder.generate_keypair().ok()?;
    let mut initiator = initiator_builder
      .local_private_key(&initiator_keys.private)
      .ok()?
      .remote_public_key(&responder_keys.public)
      .ok()?
      .build_initiator()
      .ok()?;
    let mut responder = responder_builder
      .local_private_key(&responder_keys.private)
      .ok()?
      .build_responder()
      .ok()?;
    let mut message = [0; 1024];
    let mut payload = [0; 1024];

    let length = initiator.write_message(&[], &mut message).ok()?;
    responder
      .read_message(&message[..length], &mut payload)
      .ok()?;
    let length = responder.write_message(&[], &mut message).ok()?;
    initiator
      .read_message(&message[..length], &mut payload)
      .ok()?;
    let length = initiator.write_message(&[], &mut message).ok()?;
    responder
      .read_message(&message[..length], &mut payload)
      .ok()?;

    let mut initiator_transport = initiator.into_transport_mode().ok()?;
    let mut responder_transport = responder.into_transport_mode().ok()?;

    let initiator_message = b"initiator-to-responder";
    let length = initiator_transport
      .write_message(initiator_message, &mut message)
      .ok()?;
    let received_length = responder_transport
      .read_message(&message[..length], &mut payload)
      .ok()?;
    if &payload[..received_length] != initiator_message {
      return None;
    }

    let responder_message = b"responder-to-initiator";
    let length = responder_transport
      .write_message(responder_message, &mut message)
      .ok()?;
    let received_length = initiator_transport
      .read_message(&message[..length], &mut payload)
      .ok()?;
    (&payload[..received_length] == responder_message).then_some(())
  })()
  .is_some()
}

fn run_p256_selfcheck() -> bool {
  const TEST_PRIVATE_KEY: [u8; 32] = [1; 32];
  const MESSAGE: &[u8] = b"meshpay core selfcheck";

  let Ok(signing_key) = SigningKey::from_bytes(&TEST_PRIVATE_KEY.into()) else {
    return false;
  };
  let signature: Signature = signing_key.sign(MESSAGE);
  signing_key
    .verifying_key()
    .verify(MESSAGE, &signature)
    .is_ok()
}

#[cfg(test)]
mod tests {
  use super::{CoreSelfcheck, core_selfcheck, echo};
  use std::fs;

  #[test]
  fn echo_round_trips_25_kilobytes() {
    let input = (0..25 * 1024)
      .map(|index| (index % 256) as u8)
      .collect::<Vec<_>>();

    assert_eq!(echo(input.clone()), input);
  }

  #[test]
  fn core_selfcheck_checks_sqlcipher_noise_and_p256() {
    let storage_dir = tempfile::tempdir().expect("create storage directory");
    let result: CoreSelfcheck = core_selfcheck(storage_dir.path().to_string_lossy().into_owned());

    assert!(result.sqlcipher_opens_with_key);
    assert!(result.sqlcipher_rejects_without_key);
    assert!(result.noise_xk_handshake_ok);
    assert!(result.p256_sign_verify_ok);
    assert!(!result.rust_version.is_empty());
    assert!(
      fs::read_dir(storage_dir.path())
        .expect("read storage directory")
        .next()
        .is_none()
    );
  }
}
