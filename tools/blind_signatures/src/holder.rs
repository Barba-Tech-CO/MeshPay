use blind_rsa_signatures::reexports::rand::CryptoRng;
use blind_rsa_signatures::{
  BlindMessage, BlindSignature, BlindingResult, Error, PublicKeySha384PSSRandomized,
};

use crate::signed_stub_token::SignedStubToken;
use crate::stub_token_body::StubTokenBody;

/// Client side: holds the blinding secret between request and finalize.
pub struct PendingToken {
  body: StubTokenBody,
  blinding: BlindingResult,
}

impl PendingToken {
  pub fn blind<R: CryptoRng + ?Sized>(
    rng: &mut R,
    public_key: &PublicKeySha384PSSRandomized,
    body: StubTokenBody,
  ) -> Result<Self, Error> {
    let blinding = public_key.blind(rng, body.to_bytes())?;
    Ok(Self { body, blinding })
  }

  /// The only value the issuer sees at issuance.
  pub fn blind_message(&self) -> &BlindMessage {
    &self.blinding.blind_message
  }

  /// Unblinds and verifies; fails on a wrong key or a tampered blind signature.
  pub fn finalize(
    self,
    public_key: &PublicKeySha384PSSRandomized,
    blind_signature: &BlindSignature,
  ) -> Result<SignedStubToken, Error> {
    let signature = public_key.finalize(blind_signature, &self.blinding, self.body.to_bytes())?;
    let randomizer = self.blinding.msg_randomizer.ok_or(Error::InternalError)?;
    Ok(SignedStubToken {
      body: self.body,
      randomizer,
      signature,
    })
  }

  /// Exposes the blinding secret; exists only for the leaked-secret control test.
  #[cfg(feature = "leak-blinding-secret")]
  pub fn blinding(&self) -> &BlindingResult {
    &self.blinding
  }
}
