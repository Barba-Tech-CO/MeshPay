//! A share is encrypted to the guardian's P-256 key (ephemeral ECDH + HKDF-SHA256 +
//! ChaCha20-Poly1305) and the envelope is signed by the owner's payment key (PRD 9.2).
//! The AEAD binds account, guardian and epoch; the HKDF info binds both public keys, so a
//! share cannot be replayed elsewhere or across epochs.
use crate::codec::{put, put_u64};
use crate::error::RecoveryError;
use crate::keys;
use chacha20poly1305::{
  ChaCha20Poly1305, KeyInit,
  aead::{Aead, Payload},
};
use hkdf::Hkdf;
use p256::{
  PublicKey, SecretKey,
  ecdh::diffie_hellman,
  ecdsa::{SigningKey, VerifyingKey},
};
use sha2::Sha256;
use zeroize::Zeroize;

const LABEL: &[u8] = b"sealed-share-v1";

pub struct SealedShare {
  pub account_id: String,
  pub guardian_id: String,
  pub epoch: u64,
  pub ephemeral_public: Vec<u8>,
  pub nonce: [u8; 12],
  pub ciphertext: Vec<u8>,
  pub owner_signature: Vec<u8>,
}

impl SealedShare {
  fn aad(&self) -> Vec<u8> {
    let mut buf = Vec::new();
    put(&mut buf, self.account_id.as_bytes());
    put(&mut buf, self.guardian_id.as_bytes());
    put_u64(&mut buf, self.epoch);
    put(&mut buf, &self.ephemeral_public);
    buf
  }

  /// Domain-separated bytes covered by the owner signature.
  pub fn signed_bytes(&self) -> Vec<u8> {
    let mut buf = Vec::new();
    put(&mut buf, LABEL);
    buf.extend_from_slice(&self.aad());
    put(&mut buf, &self.nonce);
    put(&mut buf, &self.ciphertext);
    buf
  }
}

fn point(key: &VerifyingKey) -> Vec<u8> {
  key.to_sec1_point(true).as_bytes().to_vec()
}

fn cipher(
  secret: &SecretKey,
  peer: &PublicKey,
  ephemeral_public: &[u8],
  guardian_public: &[u8],
) -> Result<ChaCha20Poly1305, RecoveryError> {
  let shared = diffie_hellman(secret.to_nonzero_scalar(), peer.as_affine());
  let mut info = Vec::new();
  put(&mut info, LABEL);
  put(&mut info, ephemeral_public);
  put(&mut info, guardian_public);
  let mut key = [0u8; 32];
  let derived = Hkdf::<Sha256>::new(None, shared.raw_secret_bytes().as_slice())
    .expand(&info, &mut key)
    .map_err(|_| RecoveryError::BadKey);
  let cipher =
    derived.and_then(|_| ChaCha20Poly1305::new_from_slice(&key).map_err(|_| RecoveryError::BadKey));
  key.zeroize();
  cipher
}

pub fn seal(
  share: &[u8],
  account_id: &str,
  guardian_id: &str,
  epoch: u64,
  guardian_public: &VerifyingKey,
  owner_key: &SigningKey,
) -> Result<SealedShare, RecoveryError> {
  let ephemeral = SecretKey::from(keys::random_signing_key());
  let peer = PublicKey::from(guardian_public);
  let mut nonce = [0u8; 12];
  getrandom::fill(&mut nonce).map_err(|_| RecoveryError::BadKey)?;
  let mut sealed = SealedShare {
    account_id: account_id.to_owned(),
    guardian_id: guardian_id.to_owned(),
    epoch,
    ephemeral_public: ephemeral.public_key().to_sec1_bytes().to_vec(),
    nonce,
    ciphertext: Vec::new(),
    owner_signature: Vec::new(),
  };
  let aad = sealed.aad();
  sealed.ciphertext = cipher(
    &ephemeral,
    &peer,
    &sealed.ephemeral_public,
    &point(guardian_public),
  )?
  .encrypt(
    (&nonce).into(),
    Payload {
      msg: share,
      aad: &aad,
    },
  )
  .map_err(|_| RecoveryError::Decrypt)?;
  sealed.owner_signature = keys::sign(owner_key, &sealed.signed_bytes());
  Ok(sealed)
}

/// Guardian side: owner signature first, then the expected account/guardian/epoch (an old-epoch
/// share is rejected), then decrypt.
pub fn open(
  sealed: &SealedShare,
  guardian_key: &SigningKey,
  owner_public: &VerifyingKey,
  account_id: &str,
  guardian_id: &str,
  epoch: u64,
) -> Result<Vec<u8>, RecoveryError> {
  keys::verify(
    owner_public,
    &sealed.signed_bytes(),
    &sealed.owner_signature,
  )?;
  if sealed.account_id != account_id || sealed.guardian_id != guardian_id || sealed.epoch != epoch {
    return Err(RecoveryError::WrongContext);
  }
  let ephemeral =
    PublicKey::from_sec1_bytes(&sealed.ephemeral_public).map_err(|_| RecoveryError::BadKey)?;
  let secret = SecretKey::from(guardian_key.clone());
  let aad = sealed.aad();
  cipher(
    &secret,
    &ephemeral,
    &sealed.ephemeral_public,
    &point(guardian_key.verifying_key()),
  )?
  .decrypt(
    (&sealed.nonce).into(),
    Payload {
      msg: &sealed.ciphertext,
      aad: &aad,
    },
  )
  .map_err(|_| RecoveryError::Decrypt)
}
