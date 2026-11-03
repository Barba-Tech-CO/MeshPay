use blind_rsa_signatures::{Error, MessageRandomizer, PublicKeySha384PSSRandomized, Signature};

use crate::stub_token_body::{BODY_LEN, StubTokenBody};

/// What the holder keeps and later presents: body, 32-byte randomizer and the
/// unblinded signature. The signature proves only "this issuer key signed these
/// bytes"; it says nothing about whether the body fields are reasonable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedStubToken {
  pub body: StubTokenBody,
  pub randomizer: MessageRandomizer,
  pub signature: Signature,
}

impl SignedStubToken {
  /// Signature check only. Does NOT bound value or validity: those are
  /// holder-chosen (the issuer signed blind).
  pub fn verify_signature(&self, public_key: &PublicKeySha384PSSRandomized) -> Result<(), Error> {
    public_key.verify(&self.signature, Some(self.randomizer), self.body.to_bytes())
  }

  /// Signature check plus the tier/window limits that belong to this issuer
  /// key. Limits are enforced only here, by the verifier, because the issuer
  /// uses one key per tier/window and cannot read the body.
  pub fn verify(
    &self,
    public_key: &PublicKeySha384PSSRandomized,
    expected_max_value_cents: u64,
    expected_valid_until_unix: u64,
  ) -> Result<(), Error> {
    if self.body.max_value_cents != expected_max_value_cents
      || self.body.valid_until_unix != expected_valid_until_unix
    {
      return Err(Error::VerificationFailed);
    }
    self.verify_signature(public_key)
  }

  pub fn wire_len(&self) -> usize {
    BODY_LEN + self.randomizer.0.len() + self.signature.len()
  }
}
