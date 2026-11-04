//! LAB ONLY: `lab-key-cert <root_seed_file> <server_seed_file> <days> [snapshot]` prints the root
//! public key hex and a root-signed ISSUER (or SNAPSHOT) `ServerKeyCert` hex for the key, valid from now. The real
//! root stays offline and never runs this on a connected machine.
use crate::config::app_config::read_secret_file;
use crate::crypto::p256_keys::{compressed_public_key, signing_key_from_seed};
use crate::crypto::server_key_cert::{SECONDS_PER_DAY, issue_cert};
use crate::wire::server_key_role::ServerKeyRole;
use p256::ecdsa::SigningKey;
use prost::Message;

fn read_key(path: &str) -> Result<SigningKey, String> {
  let seed = hex::decode(read_secret_file(path)?).map_err(|_| format!("{path}: not hex"))?;
  signing_key_from_seed(&seed)
}

pub fn print_lab_key_cert(args: &[String]) -> Result<(), String> {
  let (root_file, server_file, days, role) = match args {
    [root, server, days] => (root, server, days, ServerKeyRole::Issuer),
    [root, server, days, role] if role == "snapshot" => {
      (root, server, days, ServerKeyRole::Snapshot)
    }
    _ => {
      return Err(
        "usage: lab-key-cert <root_seed_file> <server_seed_file> <days> [snapshot]".to_owned(),
      );
    }
  };
  let (root, server) = (read_key(root_file)?, read_key(server_file)?);
  let days: u64 = days
    .parse()
    .map_err(|_| "days is not a number".to_owned())?;
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_err(|e| e.to_string())?
    .as_secs();
  let cert = issue_cert(&root, &server, role, now, now + days * SECONDS_PER_DAY);
  println!("{}", hex::encode(compressed_public_key(&root)));
  println!("{}", hex::encode(cert.encode_to_vec()));
  Ok(())
}
