//! PV-RISK-01: Monte Carlo of offline cheque double-spend loss (PRD v3, 5.3, 5.4, 12.1).
//! Std only. Every parameter is [ILU] from the PRD unless noted.

/// Mean time between honest receivers meeting the attacker, hours [ILU, PRD 5.4: k = E / 2 h].
pub const ENCOUNTER_H: f64 = 2.0;
/// Max cheques per chequebook per cycle [DD, PRD 5.3].
pub const N_MAX: u32 = 10;

#[derive(Clone, Copy)]
pub struct Profile {
  pub name: &'static str,
  pub reserve: f64,
  pub cheque: f64,
  pub validity_h: f64,
}

/// PRD 5.4 table, level ANDROID_STRONGBOX with single_use_key (m = 1).
pub const PROFILES: [Profile; 3] = [
  Profile {
    name: "Conservative",
    reserve: 50.0,
    cheque: 5.0,
    validity_h: 24.0,
  },
  Profile {
    name: "Moderate",
    reserve: 100.0,
    cheque: 20.0,
    validity_h: 72.0,
  },
  Profile {
    name: "Liberal",
    reserve: 500.0,
    cheque: 100.0,
    validity_h: 168.0,
  },
];

/// Attestation multipliers m, all non-one values [ILU] (PRD 5.3).
pub const LEVELS: [(&str, f64); 3] = [
  ("StrongBox single-use [ILU m=1]", 1.0),
  ("StrongBox multi-use [ILU m=0.5]", 0.5),
  ("TEE/iOS/NONE [m=0]", 0.0),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Arrival {
  /// One receiver every `ENCOUNTER_H` hours, deterministic (reproduces PRD 5.4 exactly).
  Fixed,
  /// Poisson process with the same mean rate.
  Poisson,
}

/// Time until the server revokes the payer (sync delay), hours.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Detect {
  /// Never within the cheque validity (PRD baseline).
  Never,
  Fixed(f64),
  Exp(f64),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Strategy {
  /// Deliberate payer, one acceptance per distinct cheque of the book (PRD 5.4 formula).
  Spread,
  /// Same, but receivers stop at `N_MAX` acceptances (PRD "with N_max" column).
  SpreadCapped,
  /// Deliberate payer reissues ONE cheque_id/counter to every receiver; `N_MAX` does not apply.
  Reissue,
  /// Honest payer bug: each cheque of the book is signed twice with this probability [I, free parameter].
  HonestBug(f64),
}

#[derive(Clone, Copy)]
pub struct Params {
  pub profile: Profile,
  pub m: f64,
  pub arrival: Arrival,
  pub detect: Detect,
  pub strategy: Strategy,
}

/// SplitMix64: deterministic, seeded, no wall clock.
pub struct Rng(u64);

impl Rng {
  pub fn new(seed: u64) -> Self {
    Rng(seed)
  }

  pub fn next_u64(&mut self) -> u64 {
    self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = self.0;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
  }

  /// Uniform in the open interval (0, 1).
  pub fn unit(&mut self) -> f64 {
    ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
  }

  pub fn exp(&mut self, mean: f64) -> f64 {
    -mean * self.unit().ln()
  }
}

fn arrivals(arrival: Arrival, window_h: f64, rng: &mut Rng) -> u32 {
  match arrival {
    Arrival::Fixed => (window_h / ENCOUNTER_H).floor() as u32,
    Arrival::Poisson => {
      let (mut t, mut k) = (rng.exp(ENCOUNTER_H), 0);
      while t < window_h {
        k += 1;
        t += rng.exp(ENCOUNTER_H);
      }
      k
    }
  }
}

/// Receiver loss (units U) of one attack cycle.
pub fn simulate(p: &Params, rng: &mut Rng) -> f64 {
  if p.m == 0.0 {
    return 0.0; // level does not emit cheques
  }
  let (f, r) = (p.m * p.profile.cheque, p.m * p.profile.reserve);
  if let Strategy::HonestBug(prob) = p.strategy {
    let n = (r / f).round() as u32;
    return (0..n).filter(|_| rng.unit() < prob).count() as f64 * f;
  }
  let detected_at = match p.detect {
    Detect::Never => f64::INFINITY,
    Detect::Fixed(h) => h,
    Detect::Exp(mean) => rng.exp(mean),
  };
  let k = arrivals(p.arrival, p.profile.validity_h.min(detected_at), rng);
  match p.strategy {
    Strategy::Spread => (k as f64 * f - r).max(0.0),
    Strategy::SpreadCapped => (k.min(N_MAX) as f64 * f - r).max(0.0),
    Strategy::Reissue => k.saturating_sub(1) as f64 * f,
    Strategy::HonestBug(_) => unreachable!(),
  }
}

pub struct Stats {
  pub mean: f64,
  pub p95: f64,
  pub max: f64,
}

pub fn run(p: &Params, runs: usize, seed: u64) -> Stats {
  // One sub-stream per run, so a run's draws do not shift the next run (keeps common random numbers across parameters).
  let mut seeds = Rng::new(seed);
  let mut losses: Vec<f64> = (0..runs)
    .map(|_| simulate(p, &mut Rng::new(seeds.next_u64())))
    .collect();
  losses.sort_by(f64::total_cmp);
  let rank = ((runs as f64 * 0.95).ceil() as usize).clamp(1, runs);
  Stats {
    mean: losses.iter().sum::<f64>() / runs as f64,
    p95: losses[rank - 1],
    max: losses[runs - 1],
  }
}

/// Exact PRD 5.4 loss for `Arrival::Fixed` with `Detect::Never`; for `HonestBug` this is the
/// expected loss, not a bound.
pub fn prd_loss(p: &Params) -> f64 {
  if p.m == 0.0 {
    return 0.0;
  }
  let (f, r) = (p.m * p.profile.cheque, p.m * p.profile.reserve);
  let k = (p.profile.validity_h / ENCOUNTER_H).floor();
  match p.strategy {
    Strategy::Spread => (k * f - r).max(0.0),
    Strategy::SpreadCapped => (k.min(N_MAX as f64) * f - r).max(0.0),
    Strategy::Reissue => (k - 1.0).max(0.0) * f,
    Strategy::HonestBug(prob) => (r / f).round() * prob * f,
  }
}

/// Analytic upper bound for `Arrival::Fixed` (any detection): k <= floor(E / 2 h).
pub fn analytic_bound(p: &Params) -> f64 {
  let (f, r) = (p.m * p.profile.cheque, p.m * p.profile.reserve);
  let k = (p.profile.validity_h / ENCOUNTER_H).floor();
  match p.strategy {
    _ if p.m == 0.0 => 0.0,
    Strategy::Spread => (k * f - r).max(0.0),
    Strategy::SpreadCapped => (k.min(N_MAX as f64) * f - r).max(0.0),
    Strategy::Reissue => (k - 1.0).max(0.0) * f,
    Strategy::HonestBug(_) => r, // n cheques, each loses at most f
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn params(profile: Profile, m: f64, strategy: Strategy) -> Params {
    Params {
      profile,
      m,
      arrival: Arrival::Fixed,
      detect: Detect::Never,
      strategy,
    }
  }

  fn max_loss(p: &Params) -> f64 {
    run(p, 1000, 1).max
  }

  #[test]
  fn reproduces_prd_table_exactly() {
    // PRD 5.4: (without N_max, with N_max) per profile, m = 1 then m = 0.5.
    let expected = [
      (1.0, [(10.0, 0.0), (620.0, 100.0), (7900.0, 500.0)]),
      (0.5, [(5.0, 0.0), (310.0, 50.0), (3950.0, 250.0)]),
    ];
    for (m, rows) in expected {
      for (profile, (free, capped)) in PROFILES.iter().zip(rows) {
        let a = run(&params(*profile, m, Strategy::Spread), 1000, 7);
        let b = run(&params(*profile, m, Strategy::SpreadCapped), 1000, 7);
        assert_eq!((a.mean, a.p95, a.max), (free, free, free));
        assert_eq!((b.mean, b.p95, b.max), (capped, capped, capped));
      }
    }
  }

  #[test]
  fn reissue_defeats_n_max() {
    for profile in PROFILES {
      let capped = max_loss(&params(profile, 1.0, Strategy::SpreadCapped));
      let reissue = max_loss(&params(profile, 1.0, Strategy::Reissue));
      assert!(reissue > capped, "{}", profile.name);
    }
  }

  #[test]
  fn no_emission_means_no_loss() {
    for s in [
      Strategy::Spread,
      Strategy::Reissue,
      Strategy::HonestBug(0.5),
    ] {
      assert_eq!(max_loss(&params(PROFILES[2], 0.0, s)), 0.0);
    }
  }

  #[test]
  fn prd_formulas_match_in_1000_runs_for_every_profile_and_level() {
    for profile in PROFILES {
      for m in [1.0, 0.5, 0.0] {
        for strategy in [Strategy::Spread, Strategy::SpreadCapped] {
          let p = params(profile, m, strategy);
          let stats = run(&p, 1000, 11);
          let expected = prd_loss(&p);
          assert_eq!(
            (stats.mean, stats.p95, stats.max),
            (expected, expected, expected),
            "{} m={m} {strategy:?}",
            profile.name
          );
        }
      }
    }
  }

  #[test]
  fn m_zero_gives_no_loss_for_every_profile_and_cap_profile() {
    for profile in PROFILES {
      for strategy in [
        Strategy::Spread,
        Strategy::SpreadCapped,
        Strategy::Reissue,
        Strategy::HonestBug(1.0),
      ] {
        let stats = run(&params(profile, 0.0, strategy), 1000, 3);
        assert_eq!(
          (stats.mean, stats.p95, stats.max),
          (0.0, 0.0, 0.0),
          "{} {strategy:?}",
          profile.name
        );
      }
    }
  }

  #[test]
  fn deterministic_by_seed() {
    let mut p = params(PROFILES[1], 1.0, Strategy::Reissue);
    p.arrival = Arrival::Poisson;
    p.detect = Detect::Exp(24.0);
    let (a, b, c) = (run(&p, 500, 42), run(&p, 500, 42), run(&p, 500, 43));
    assert_eq!((a.mean, a.p95, a.max), (b.mean, b.p95, b.max));
    assert_ne!(a.mean, c.mean);
  }

  #[test]
  fn loss_within_analytic_bound() {
    let strategies = [
      Strategy::Spread,
      Strategy::SpreadCapped,
      Strategy::Reissue,
      Strategy::HonestBug(0.3),
    ];
    for profile in PROFILES {
      for m in [1.0, 0.5] {
        for s in strategies {
          for detect in [Detect::Never, Detect::Fixed(10.0), Detect::Exp(30.0)] {
            let mut p = params(profile, m, s);
            p.detect = detect;
            assert!(max_loss(&p) <= analytic_bound(&p) + 1e-9);
          }
        }
      }
    }
  }

  #[test]
  fn spread_never_exceeds_reissue() {
    for profile in PROFILES {
      let spread = max_loss(&params(profile, 1.0, Strategy::Spread));
      let reissue = max_loss(&params(profile, 1.0, Strategy::Reissue));
      assert!(spread <= reissue);
    }
  }

  #[test]
  fn monotonic_in_validity_and_cheque_value() {
    for s in [Strategy::Spread, Strategy::SpreadCapped, Strategy::Reissue] {
      for arrival in [Arrival::Fixed, Arrival::Poisson] {
        let mut prev_e = 0.0;
        for e in [6.0, 12.0, 24.0, 48.0, 96.0, 192.0] {
          let mut p = params(PROFILES[1], 1.0, s);
          p.arrival = arrival;
          p.detect = Detect::Exp(60.0);
          p.profile.validity_h = e;
          let mean = run(&p, 400, 5).mean;
          assert!(mean >= prev_e, "E {s:?} {arrival:?}");
          prev_e = mean;
        }
        let mut prev_f = 0.0;
        for f in [5.0, 10.0, 20.0, 40.0, 80.0] {
          let mut p = params(PROFILES[1], 1.0, s);
          p.arrival = arrival;
          p.profile.cheque = f;
          let mean = run(&p, 400, 5).mean;
          assert!(mean >= prev_f, "f {s:?} {arrival:?}");
          prev_f = mean;
        }
      }
    }
  }

  #[test]
  fn reserve_does_not_raise_loss() {
    for s in [Strategy::Spread, Strategy::Reissue] {
      let mut prev = f64::INFINITY;
      for r in [50.0, 100.0, 200.0, 400.0] {
        let mut p = params(PROFILES[1], 1.0, s);
        p.profile.reserve = r;
        let mean = run(&p, 100, 3).mean;
        assert!(mean <= prev, "R {s:?}");
        prev = mean;
      }
    }
  }

  #[test]
  fn detection_shortens_window() {
    let free = max_loss(&params(PROFILES[1], 1.0, Strategy::Reissue));
    let mut p = params(PROFILES[1], 1.0, Strategy::Reissue);
    p.detect = Detect::Fixed(12.0);
    assert!(max_loss(&p) < free);
  }
}
