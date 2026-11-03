//! Sizes (exact) and timings (this machine, optimized test profile, single
//! thread). Run `cargo test --test measurements -- --nocapture` to see them.
mod common;

use std::time::{Duration, Instant};

use blind_signatures::holder::PendingToken;
use blind_signatures::issuer::Issuer;
use common::{random_body, seeded_rng};

const ITERATIONS: usize = 200;

fn percentile(samples: &mut [Duration], p: f64) -> Duration {
  samples.sort();
  samples[((samples.len() as f64 - 1.0) * p).round() as usize]
}

fn ms(d: Duration) -> f64 {
  d.as_secs_f64() * 1000.0
}

fn report(label: &str, samples: &mut [Duration]) {
  println!(
    "  {label:<9} median {:>8.3} ms   p95 {:>8.3} ms",
    ms(percentile(samples, 0.5)),
    ms(percentile(samples, 0.95))
  );
}

fn measure(bits: usize, keygen_runs: usize) {
  let mut keygen = Vec::new();
  let mut issuer = None;
  for run in 0..keygen_runs {
    let start = Instant::now();
    issuer = Some(Issuer::generate(&mut seeded_rng(run as u64), bits).unwrap());
    keygen.push(start.elapsed());
  }
  let issuer = issuer.unwrap();
  let pk = issuer.public_key();
  let mut rng = seeded_rng(999);
  let (mut t_blind, mut t_sign, mut t_final, mut t_verify) = (vec![], vec![], vec![], vec![]);
  let mut sizes = None;
  for _ in 0..ITERATIONS {
    let body = random_body(&mut rng);
    let s = Instant::now();
    let pending = PendingToken::blind(&mut rng, pk, body).unwrap();
    t_blind.push(s.elapsed());
    let s = Instant::now();
    let blind_signature = issuer.sign_blinded(pending.blind_message()).unwrap();
    t_sign.push(s.elapsed());
    let blind_len = pending.blind_message().0.len();
    let s = Instant::now();
    let token = pending.finalize(pk, &blind_signature).unwrap();
    t_final.push(s.elapsed());
    let s = Instant::now();
    token.verify_signature(pk).unwrap();
    t_verify.push(s.elapsed());
    sizes = Some((
      blind_len,
      blind_signature.0.len(),
      token.signature.0.len(),
      token.randomizer.0.len(),
      token.wire_len(),
    ));
  }
  let (blind_len, blind_sig_len, sig_len, rand_len, wire) = sizes.unwrap();
  let modulus_bytes = bits / 8;
  println!("RSA-{bits} (RSABSSA-SHA384-PSS-Randomized), {ITERATIONS} iterations");
  println!(
    "  sizes: blinded_msg={blind_len} B blind_sig={blind_sig_len} B signature={sig_len} B randomizer={rand_len} B body=65 B token_on_wire={wire} B spki_pubkey={} B",
    pk.to_spki().unwrap().len()
  );
  report("keygen", &mut keygen);
  report("blind", &mut t_blind);
  report("sign", &mut t_sign);
  report("finalize", &mut t_final);
  report("verify", &mut t_verify);
  assert_eq!(
    (blind_len, blind_sig_len, sig_len, rand_len),
    (modulus_bytes, modulus_bytes, modulus_bytes, 32)
  );
  assert_eq!(wire, 65 + 32 + modulus_bytes);
}

#[test]
fn rsa_2048_sizes_and_timings() {
  measure(2048, 10);
}

#[test]
fn rsa_3072_sizes_and_timings() {
  measure(3072, 3);
}
