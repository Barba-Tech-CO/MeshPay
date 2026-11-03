//! Sanity checks, NOT evidence of unlinkability. Seeded RNG: deterministic.
//! Unlinkability of RFC 9474 comes from the blinding factor being uniform,
//! which is argued by the RFC/literature, not by these tests. They can only
//! catch gross implementation failures (no blinding, reused factor).
mod common;

use blind_signatures::holder::PendingToken;
use blind_signatures::issuer::Issuer;
use common::{new_pending, random_body, seeded_rng};

const SAMPLES: usize = 2_000;
const MID_BYTES: std::ops::Range<usize> = 64..192;
// Generous bound (255 df, mean 255): a gross-failure alarm, not a p-value claim.
const CHI_SQUARE_ALARM: f64 = 370.0;

fn byte_histogram<'a>(messages: impl Iterator<Item = &'a [u8]>) -> [f64; 256] {
  let mut hist = [0f64; 256];
  for m in messages {
    for &b in &m[MID_BYTES] {
      hist[b as usize] += 1.0;
    }
  }
  hist
}

fn chi_square_uniform(hist: &[f64; 256]) -> f64 {
  let expected = hist.iter().sum::<f64>() / 256.0;
  hist.iter().map(|o| (o - expected).powi(2) / expected).sum()
}

fn chi_square_two_sample(a: &[f64; 256], b: &[f64; 256]) -> f64 {
  a.iter()
    .zip(b)
    .map(|(x, y)| {
      if x + y == 0.0 {
        0.0
      } else {
        (x - y).powi(2) / (x + y)
      }
    })
    .sum()
}

fn blinded_messages(issuer: &Issuer, body_seed: u64, rng_seed: u64) -> Vec<Vec<u8>> {
  let mut rng = seeded_rng(rng_seed);
  let body = random_body(&mut seeded_rng(body_seed));
  (0..SAMPLES)
    .map(|_| {
      let p = PendingToken::blind(&mut rng, issuer.public_key(), body.clone()).unwrap();
      p.blind_message().0.clone()
    })
    .collect()
}

/// Gross-failure check only: byte frequencies of blinded messages for two
/// different bodies look alike and uniform. Passing proves nothing about
/// unlinkability.
#[test]
fn sanity_blinded_message_bytes_show_no_gross_dependence_on_body() {
  let issuer = Issuer::generate(&mut seeded_rng(1), 2048).unwrap();
  let a = blinded_messages(&issuer, 100, 200);
  let b = blinded_messages(&issuer, 101, 201);
  let ha = byte_histogram(a.iter().map(Vec::as_slice));
  let hb = byte_histogram(b.iter().map(Vec::as_slice));
  let (ua, ub) = (chi_square_uniform(&ha), chi_square_uniform(&hb));
  let two = chi_square_two_sample(&ha, &hb);
  println!("chi2 uniform A={ua:.1} B={ub:.1}; two-sample={two:.1}; alarm={CHI_SQUARE_ALARM}");
  assert!(ua < CHI_SQUARE_ALARM && ub < CHI_SQUARE_ALARM && two < CHI_SQUARE_ALARM);
}

/// Sanity: values the issuer saw (blinded message, blind signature) never
/// reappear verbatim in the final token signature.
#[test]
fn sanity_issuance_log_values_do_not_reappear_in_tokens() {
  const N: usize = 100;
  let issuer = Issuer::generate(&mut seeded_rng(1), 2048).unwrap();
  let mut rng = seeded_rng(300);
  let mut log: Vec<Vec<u8>> = Vec::new();
  let mut tokens = Vec::new();
  for _ in 0..N {
    let pending = new_pending(&issuer, &mut rng);
    let blind_signature = issuer.sign_blinded(pending.blind_message()).unwrap();
    log.push(pending.blind_message().0.clone());
    log.push(blind_signature.0.clone());
    tokens.push(
      pending
        .finalize(issuer.public_key(), &blind_signature)
        .unwrap(),
    );
  }
  for token in &tokens {
    assert!(!log.contains(&token.signature.0));
  }
}

/// CONTROL, not a result: whoever holds the blinding secret trivially links
/// token to request. Shows the secret is exactly the link; it never leaves the
/// holder in the protocol.
#[test]
fn control_blinding_secret_links_token_to_request() {
  const N: usize = 60;
  let issuer = Issuer::generate(&mut seeded_rng(1), 2048).unwrap();
  let mut rng = seeded_rng(400);
  let mut rows = Vec::new();
  let mut tokens = Vec::new();
  for _ in 0..N {
    let pending = new_pending(&issuer, &mut rng);
    let blind_signature = issuer.sign_blinded(pending.blind_message()).unwrap();
    rows.push((pending.blinding().clone(), blind_signature.clone()));
    tokens.push(
      pending
        .finalize(issuer.public_key(), &blind_signature)
        .unwrap(),
    );
  }
  for (truth, token) in tokens.iter().enumerate() {
    let body = token.body.to_bytes();
    let matches: Vec<usize> = (0..N)
      .filter(|&j| {
        issuer
          .public_key()
          .finalize(&rows[j].1, &rows[j].0, body)
          .is_ok()
      })
      .collect();
    assert_eq!(matches, vec![truth]);
  }
}
