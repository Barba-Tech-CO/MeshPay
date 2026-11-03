//! In-process model of the server side of PRD 9.3 / 9.5. Time is a parameter (`now`, seconds)
//! so the clock can be simulated. All time/amount arithmetic is checked or saturating.
use crate::account::Account;
use crate::cheque::Cheque;
use crate::error::RecoveryError;
use crate::keys;
use crate::params::{CANCEL_BLOCK, MAX_REQUESTS_PER_WINDOW, RATE_WINDOW, T_REC};
use crate::pending::Pending;
use crate::recovery_cancel::RecoveryCancel;
use crate::recovery_cert::RecoveryCert;
use crate::reservation::Reservation;
use crate::snapshot::Snapshot;
use p256::ecdsa::{SigningKey, VerifyingKey};
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct Ledger {
  server_key: SigningKey,
  accounts: HashMap<String, Account>,
}

impl Ledger {
  pub fn new(server_key: SigningKey) -> Self {
    Self {
      server_key,
      accounts: HashMap::new(),
    }
  }

  pub fn server_public(&self) -> VerifyingKey {
    *self.server_key.verifying_key()
  }

  fn account(&mut self, id: &str) -> Result<&mut Account, RecoveryError> {
    self
      .accounts
      .get_mut(id)
      .ok_or(RecoveryError::UnknownAccount)
  }

  /// Onboarding: registers the RK public key and the epoch-1 payment key (PRD 9.2).
  pub fn register(
    &mut self,
    id: &str,
    rk_public: VerifyingKey,
    payment_public: VerifyingKey,
    balance: u64,
  ) {
    self.accounts.insert(
      id.to_owned(),
      Account {
        rk_public,
        epoch: 1,
        payment_keys: HashMap::from([(1, payment_public)]),
        balance,
        reservations: HashMap::new(),
        deposited: HashSet::new(),
        pending: None,
        request_times: Vec::new(),
        request_counter: 0,
        last_effective: None,
        blocked_until: 0,
      },
    );
  }

  pub fn epoch(&self, id: &str) -> Option<u64> {
    self.accounts.get(id).map(|account| account.epoch)
  }

  /// Request counter of the pending recovery, which the old device needs to sign a cancel.
  pub fn pending_request_no(&self, id: &str) -> Option<u64> {
    self
      .accounts
      .get(id)?
      .pending
      .as_ref()
      .map(|pending| pending.request_no)
  }

  /// Balance minus cheques still reserved (not deposited, not expired). Expiry of the last
  /// old-epoch cheque is what ends the quarantine (PRD 9.3 step 5).
  pub fn available(&self, id: &str, now: u64) -> Result<u64, RecoveryError> {
    let account = self.accounts.get(id).ok_or(RecoveryError::UnknownAccount)?;
    let reserved = account
      .reservations
      .values()
      .filter(|reservation| reservation.expiry > now)
      .fold(0u64, |sum, reservation| {
        sum.saturating_add(reservation.amount)
      });
    Ok(account.balance.saturating_sub(reserved))
  }

  /// Issues one cheque slot from the free balance. Refused while frozen (PRD 9.5 item 1), for
  /// any epoch other than the current one, and for a `cheque_id` already reserved or deposited.
  pub fn reserve(
    &mut self,
    id: &str,
    epoch: u64,
    cheque_id: u64,
    amount: u64,
    expiry: u64,
    now: u64,
  ) -> Result<(), RecoveryError> {
    let free = self.available(id, now)?;
    let account = self.account(id)?;
    if account.pending.is_some() {
      return Err(RecoveryError::Frozen);
    }
    if epoch != account.epoch {
      return Err(RecoveryError::WrongEpoch);
    }
    if account.reservations.contains_key(&cheque_id) || account.deposited.contains(&cheque_id) {
      return Err(RecoveryError::DuplicateCheque);
    }
    if expiry <= now {
      return Err(RecoveryError::Expired);
    }
    if amount > free {
      return Err(RecoveryError::InsufficientFunds);
    }
    account.reservations.insert(
      cheque_id,
      Reservation {
        amount,
        expiry,
        epoch,
      },
    );
    Ok(())
  }

  /// PRD 9.3 steps 3-4 and 9.5 items 1 and 3. Every check runs before any state changes.
  pub fn submit_recovery(&mut self, cert: RecoveryCert, now: u64) -> Result<(), RecoveryError> {
    let account = self.account(&cert.account_id)?;
    keys::verify(&account.rk_public, &cert.signed_bytes(), &cert.signature)?;
    if cert.old_epoch != account.epoch || Some(cert.new_epoch) != account.epoch.checked_add(1) {
      return Err(RecoveryError::StaleEpoch);
    }
    if account.pending.is_some() {
      return Err(RecoveryError::AlreadyPending);
    }
    VerifyingKey::from_sec1_bytes(&cert.new_device_pubkey).map_err(|_| RecoveryError::BadKey)?;
    if now < account.blocked_until {
      return Err(RecoveryError::Blocked);
    }
    account
      .request_times
      .retain(|time| now < time.saturating_add(RATE_WINDOW));
    if account.request_times.len() >= MAX_REQUESTS_PER_WINDOW {
      return Err(RecoveryError::RateLimited);
    }
    if account
      .last_effective
      .is_some_and(|time| now < time.saturating_add(RATE_WINDOW))
    {
      return Err(RecoveryError::RateLimited);
    }
    let activates_at = now.checked_add(T_REC).ok_or(RecoveryError::Overflow)?;
    let request_no = account
      .request_counter
      .checked_add(1)
      .ok_or(RecoveryError::Overflow)?;
    account.request_times.push(now);
    account.request_counter = request_no;
    account.pending = Some(Pending {
      cert,
      activates_at,
      request_no,
    });
    Ok(())
  }

  /// PRD 9.5 item 2: only the old-epoch payment key can cancel, only before `T_rec`, and only
  /// the pending request it was signed for (cert hash and request counter are recomputed here).
  pub fn cancel(&mut self, cancel: &RecoveryCancel, now: u64) -> Result<(), RecoveryError> {
    let account = self.account(&cancel.account_id)?;
    let pending = account.pending.as_ref().ok_or(RecoveryError::NoPending)?;
    if now >= pending.activates_at {
      return Err(RecoveryError::TimeLockActive);
    }
    let key = account
      .payment_keys
      .get(&account.epoch)
      .ok_or(RecoveryError::BadKey)?;
    let message = RecoveryCancel::signed_bytes(
      &cancel.account_id,
      account.epoch,
      &pending.cert,
      pending.request_no,
    );
    keys::verify(key, &message, &cancel.signature)?;
    account.pending = None;
    account.blocked_until = now.saturating_add(CANCEL_BLOCK);
    Ok(())
  }

  /// Activates the new epoch once `T_rec` has elapsed; old key loses new emissions. Pending is
  /// consumed only after every fallible step succeeded.
  pub fn activate(&mut self, id: &str, now: u64) -> Result<(), RecoveryError> {
    let account = self.account(id)?;
    let pending = account.pending.as_ref().ok_or(RecoveryError::NoPending)?;
    if now < pending.activates_at {
      return Err(RecoveryError::TimeLockActive);
    }
    let key = VerifyingKey::from_sec1_bytes(&pending.cert.new_device_pubkey)
      .map_err(|_| RecoveryError::BadKey)?;
    let new_epoch = pending.cert.new_epoch;
    account.pending = None;
    account.epoch = new_epoch;
    account.payment_keys.insert(new_epoch, key);
    account.last_effective = Some(now);
    Ok(())
  }

  /// Deposit of a cheque from any epoch: paid only if it was reserved, signed by that epoch's
  /// key and not past `E` (PRD 9.3 steps 5 and 7).
  pub fn deposit(&mut self, cheque: &Cheque, now: u64) -> Result<(), RecoveryError> {
    let account = self.account(&cheque.payer_account)?;
    if account.deposited.contains(&cheque.id) {
      return Err(RecoveryError::AlreadyDeposited);
    }
    let reservation = account
      .reservations
      .get(&cheque.id)
      .ok_or(RecoveryError::UnknownCheque)?;
    let key = account
      .payment_keys
      .get(&cheque.epoch)
      .ok_or(RecoveryError::WrongEpoch)?;
    keys::verify(key, &cheque.signed_bytes(), &cheque.signature)?;
    if cheque.epoch != reservation.epoch
      || cheque.amount != reservation.amount
      || cheque.expiry != reservation.expiry
    {
      return Err(RecoveryError::UnknownCheque);
    }
    if now > reservation.expiry {
      return Err(RecoveryError::Expired);
    }
    account.balance = account
      .balance
      .checked_sub(cheque.amount)
      .ok_or(RecoveryError::InsufficientFunds)?;
    account.reservations.remove(&cheque.id);
    account.deposited.insert(cheque.id);
    Ok(())
  }

  /// Server-signed snapshot (PRD 9.2 metadata only).
  pub fn snapshot(&self) -> Snapshot {
    let accounts: BTreeMap<String, (u64, Vec<u8>)> = self
      .accounts
      .iter()
      .filter_map(|(id, account)| {
        let key = account.payment_keys.get(&account.epoch)?;
        Some((
          id.clone(),
          (account.epoch, key.to_sec1_point(true).as_bytes().to_vec()),
        ))
      })
      .collect();
    let mut pending_recovery_accounts: Vec<String> = self
      .accounts
      .iter()
      .filter(|(_, account)| account.pending.is_some())
      .map(|(id, _)| id.clone())
      .collect();
    pending_recovery_accounts.sort();
    Snapshot {
      accounts,
      pending_recovery_accounts,
      signature: Vec::new(),
    }
    .sign(&self.server_key)
  }
}
