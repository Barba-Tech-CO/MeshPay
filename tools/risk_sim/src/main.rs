use risk_sim::{Arrival, Detect, LEVELS, N_MAX, PROFILES, Params, Profile, Stats, Strategy, run};

const USAGE: &str = "usage: risk_sim [--runs N] [--seed S] [--arrival fixed|poisson] \
[--detect never|fixed:H|exp:H] [--bug-rate P]";

struct Args {
  runs: usize,
  seed: u64,
  arrival: Arrival,
  detect: Detect,
  bug_rate: f64,
}

fn parse() -> Result<Args, String> {
  let mut a = Args {
    runs: 1000,
    seed: 1,
    arrival: Arrival::Fixed,
    detect: Detect::Never,
    bug_rate: 0.05,
  };
  let mut it = std::env::args().skip(1);
  while let Some(flag) = it.next() {
    let v = it.next().ok_or(USAGE)?;
    let bad = |e: &dyn std::fmt::Display| format!("{flag} {v}: {e}\n{USAGE}");
    match flag.as_str() {
      "--runs" => a.runs = v.parse().map_err(|e| bad(&e))?,
      "--seed" => a.seed = v.parse().map_err(|e| bad(&e))?,
      "--bug-rate" => a.bug_rate = v.parse().map_err(|e| bad(&e))?,
      "--arrival" => {
        a.arrival = match v.as_str() {
          "fixed" => Arrival::Fixed,
          "poisson" => Arrival::Poisson,
          _ => return Err(bad(&"unknown arrival")),
        }
      }
      "--detect" => {
        a.detect = match v.split_once(':') {
          None if v == "never" => Detect::Never,
          Some(("fixed", h)) => Detect::Fixed(h.parse().map_err(|e| bad(&e))?),
          Some(("exp", h)) => Detect::Exp(h.parse().map_err(|e| bad(&e))?),
          _ => return Err(bad(&"unknown detect")),
        }
      }
      _ => return Err(USAGE.into()),
    }
  }
  if a.runs == 0 {
    return Err(format!("--runs must be > 0\n{USAGE}"));
  }
  Ok(a)
}

fn fmt(s: &Stats) -> String {
  format!("{:>8.1}/{:>8.1}/{:>8.1}", s.mean, s.p95, s.max)
}

fn main() {
  let a = match parse() {
    Ok(a) => a,
    Err(e) => {
      eprintln!("{e}");
      std::process::exit(2);
    }
  };
  println!(
    "PV-RISK-01 | runs={} seed={} arrival={:?} detect={:?} N_max={} | cells: mean/p95/max loss in U [ILU]",
    a.runs, a.seed, a.arrival, a.detect, N_MAX
  );
  let strategies = [
    ("PRD no N_max", Strategy::Spread),
    ("PRD N_max", Strategy::SpreadCapped),
    ("reissue 1 id", Strategy::Reissue),
    ("honest bug [I]", Strategy::HonestBug(a.bug_rate)),
  ];
  let at = |profile: Profile, m: f64, strategy| {
    run(
      &Params {
        profile,
        m,
        arrival: a.arrival,
        detect: a.detect,
        strategy,
      },
      a.runs,
      a.seed,
    )
  };
  for (level, m) in LEVELS {
    println!("\n== {level} ==");
    print!("{:<13}", "profile");
    for (name, _) in strategies {
      print!(" | {name:^26}");
    }
    println!();
    for profile in PROFILES {
      print!("{:<13}", profile.name);
      for (_, s) in strategies {
        print!(" | {}", fmt(&at(profile, m, s)));
      }
      println!();
    }
  }
  println!("\n== Sensitivity (Moderate, m=1): mean loss when ONE parameter doubles ==");
  let base = PROFILES[1];
  let variants: [(&str, Profile); 4] = [
    ("baseline", base),
    (
      "E x2",
      Profile {
        validity_h: base.validity_h * 2.0,
        ..base
      },
    ),
    (
      "f x2",
      Profile {
        cheque: base.cheque * 2.0,
        ..base
      },
    ),
    (
      "R x2",
      Profile {
        reserve: base.reserve * 2.0,
        ..base
      },
    ),
  ];
  println!(
    "{:<10} | {:>9} {:>6} | {:>9} {:>6}",
    "variant", "PRD", "ratio", "reissue", "ratio"
  );
  let b = (
    at(base, 1.0, Strategy::Spread).mean,
    at(base, 1.0, Strategy::Reissue).mean,
  );
  for (name, p) in variants {
    let (s, r) = (
      at(p, 1.0, Strategy::Spread).mean,
      at(p, 1.0, Strategy::Reissue).mean,
    );
    println!(
      "{name:<10} | {s:>9.1} {:>6.2} | {r:>9.1} {:>6.2}",
      s / b.0,
      r / b.1
    );
  }
}
