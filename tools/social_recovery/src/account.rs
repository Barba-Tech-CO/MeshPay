use crate::pending::Pending;
use crate::reservation::Reservation;
use p256::ecdsa::VerifyingKey;
use std::collections::{HashMap, HashSet};

pub struct Account {
  pub rk_public: VerifyingKey,
  pub epoch: u64,
  pub payment_keys: HashMap<u64, VerifyingKey>,
  pub balance: u64,
  pub reservations: HashMap<u64, Reservation>,
  pub deposited: HashSet<u64>,
  pub pending: Option<Pending>,
  pub request_times: Vec<u64>,
  pub request_counter: u64,
  pub last_effective: Option<u64>,
  pub blocked_until: u64,
}
