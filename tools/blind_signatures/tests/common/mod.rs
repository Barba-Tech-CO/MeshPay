#![allow(dead_code)]
use blind_signatures::stub_token_body::StubTokenBody;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

pub fn seeded_rng(seed: u64) -> ChaCha20Rng {
  ChaCha20Rng::seed_from_u64(seed)
}

pub fn random_body(rng: &mut ChaCha20Rng) -> StubTokenBody {
  let mut body = StubTokenBody {
    cheque_id: [0; 16],
    cheque_public_key: [0; 33],
    max_value_cents: 5_000,
    valid_until_unix: 1_800_000_000,
  };
  rng.fill_bytes(&mut body.cheque_id);
  rng.fill_bytes(&mut body.cheque_public_key);
  body
}

pub fn new_pending(
  issuer: &blind_signatures::issuer::Issuer,
  rng: &mut ChaCha20Rng,
) -> blind_signatures::holder::PendingToken {
  let body = random_body(rng);
  blind_signatures::holder::PendingToken::blind(rng, issuer.public_key(), body).unwrap()
}
