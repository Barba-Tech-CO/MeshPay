use blind_rsa_signatures::reexports::rand::CryptoRng;
use blind_rsa_signatures::{
  BlindMessage, BlindSignature, Error, KeyPairSha384PSSRandomized, PublicKeySha384PSSRandomized,
};

/// Server side of RFC 9474 (RSABSSA-SHA384-PSS-Randomized). The issuer signs
/// blind and enforces nothing about the token body. Value tier and validity
/// window are tied to the KEY (one key per tier/window) and checked by the
/// verifier against that key's known parameters.
pub struct Issuer {
  key_pair: KeyPairSha384PSSRandomized,
}

impl Issuer {
  pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R, modulus_bits: usize) -> Result<Self, Error> {
    Ok(Self {
      key_pair: KeyPairSha384PSSRandomized::generate(rng, modulus_bits)?,
    })
  }

  pub fn public_key(&self) -> &PublicKeySha384PSSRandomized {
    &self.key_pair.pk
  }

  pub fn sign_blinded(&self, blind_message: &BlindMessage) -> Result<BlindSignature, Error> {
    self.key_pair.sk.blind_sign(blind_message)
  }
}
