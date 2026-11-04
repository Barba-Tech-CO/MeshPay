//! PRD 4.3 items 3 and 5, 8.1: `ReceiverEngine::store_propose` applies `ChequeAcceptance` to the
//! stub expiry with the injected protocol clock, and the bridge caps DTN traffic per Noise pair.
mod support;

use meshpay_core::acceptance_context::AcceptanceContext;
use meshpay_core::api::{
  DtnBuffer, NoiseSession, ReceiverTxEngine, build_dtn_envelope, generate_noise_static_keypair,
};
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::protocol_now::ProtocolNow;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use prost::Message;
use support::{E, T_SRV};

const PAYER_SEED: [u8; 32] = [7; 32];
const PAYEE_SEED: [u8; 32] = [9; 32];
const DB_KEY: [u8; 32] = [3; 32];
const OWN_ACCOUNT: &[u8] = b"payee-account";
const SECOND: u64 = 1_000_000_000;

struct Pair {
  _directory: tempfile::TempDir,
  payer: PayerEngine,
  receiver: ReceiverEngine,
}

impl Pair {
  fn new() -> Self {
    let directory = tempfile::tempdir().unwrap();
    let public = |seed: &[u8; 32]| compressed_public_key(&signing_key_from_seed(seed).unwrap());
    Self {
      payer: PayerEngine::open(
        &directory.path().join("p.db"),
        &DB_KEY,
        &PAYER_SEED,
        &public(&PAYEE_SEED),
      )
      .unwrap(),
      receiver: ReceiverEngine::open(
        &directory.path().join("r.db"),
        &DB_KEY,
        &PAYEE_SEED,
        &public(&PAYER_SEED),
        OWN_ACCOUNT,
        support::issuer_trust(),
      )
      .unwrap(),
      _directory: directory,
    }
  }

  /// Propose for a fresh cheque expiring at `expires_at_unix`.
  fn propose(&self, index: u64, expires_at_unix: u64) -> Vec<u8> {
    let body = ChequeSpendBody {
      cheque_id: format!("cheque-{index:08}").into_bytes(),
      payee_account_id: OWN_ACCOUNT.to_vec(),
      amount_units: 100,
      counter: index,
      ..Default::default()
    }
    .encode_to_vec();
    let stub = support::stub_expiring(&format!("cheque-{index:08}").into_bytes(), expires_at_unix);
    self
      .payer
      .propose(&body, index, OWN_ACCOUNT, &stub)
      .unwrap()
  }

  fn store(
    &self,
    index: u64,
    expires_at_unix: u64,
    context: AcceptanceContext,
  ) -> Result<Vec<u8>, String> {
    self
      .receiver
      .store_propose(&self.propose(index, expires_at_unix), &context)
  }
}

fn at(unix: u64, rebooted: bool) -> AcceptanceContext {
  AcceptanceContext {
    now: ProtocolNow {
      unix,
      t_srv_unix: T_SRV,
      rebooted,
    },
    ..support::context()
  }
}

#[test]
fn valid_cheque_is_stored() {
  assert!(Pair::new().store(1, T_SRV + E, support::context()).is_ok());
}

#[test]
fn expiry_e_outside_24_to_168_hours_is_refused() {
  let pair = Pair::new();
  for (index, expiry_secs) in [(1, 23 * 3600), (2, 169 * 3600)] {
    let context = AcceptanceContext {
      expiry_secs,
      ..support::context()
    };
    assert_eq!(
      pair.store(index, T_SRV + 3600, context),
      Err("expiry E must be 24 to 168 hours".to_owned())
    );
  }
}

#[test]
fn missing_snapshot_is_refused() {
  let context = AcceptanceContext {
    snapshot_issued_at_unix: None,
    ..support::context()
  };
  assert_eq!(
    Pair::new().store(1, T_SRV + 3600, context),
    Err("no revocation snapshot".to_owned())
  );
}

#[test]
fn snapshot_older_than_s_max_is_refused() {
  assert_eq!(
    Pair::new().store(1, T_SRV + E + 3600, at(T_SRV + E + 1, false)),
    Err("revocation snapshot older than S_max".to_owned())
  );
}

#[test]
fn snapshot_more_than_60_s_in_the_future_is_refused() {
  let context = AcceptanceContext {
    snapshot_issued_at_unix: Some(T_SRV + 61),
    ..support::context()
  };
  assert_eq!(
    Pair::new().store(1, T_SRV + 3600, context),
    Err("revocation snapshot issued in the future".to_owned())
  );
}

#[test]
fn expired_cheque_is_refused() {
  assert_eq!(
    Pair::new().store(1, T_SRV + 10, at(T_SRV + 10, false)),
    Err("cheque expired".to_owned())
  );
}

#[test]
fn after_reboot_expiry_beyond_e_of_t_srv_is_refused() {
  let pair = Pair::new();
  assert_eq!(
    pair.store(1, T_SRV + E + 1, at(T_SRV, true)),
    Err("after reboot only cheques expiring within E of t_srv are accepted".to_owned())
  );
  assert!(pair.store(2, T_SRV + E, at(T_SRV, true)).is_ok());
}

#[test]
fn stored_cheque_redelivered_after_expiry_keeps_its_tx_id() {
  let pair = Pair::new();
  let propose = pair.propose(1, T_SRV + 100);
  let tx_id = pair
    .receiver
    .store_propose(&propose, &support::context())
    .unwrap();
  assert_eq!(
    pair
      .receiver
      .store_propose(&propose, &at(T_SRV + 200, false)),
    Ok(tx_id)
  );
}

#[test]
fn bridge_receiver_refuses_until_a_verified_snapshot_anchors_the_clock() {
  let directory = tempfile::tempdir().unwrap();
  let pair = Pair::new();
  let receiver = ReceiverTxEngine::open(
    directory.path().join("r.db").to_string_lossy().into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&PAYER_SEED).unwrap()),
    OWN_ACCOUNT.to_vec(),
    support::snapshot_public_key(),
    E,
    support::issuer_root_public_key(),
    support::issuer_cert(),
  )
  .unwrap();
  let propose = pair.propose(1, T_SRV + 3600);
  assert_eq!(
    receiver.on_propose(propose.clone(), 10 * SECOND, Some(1)),
    Err("no revocation snapshot".to_owned())
  );
  let mut forged = support::signed_snapshot(T_SRV);
  *forged.last_mut().unwrap() ^= 1;
  assert!(
    receiver
      .install_snapshot(forged, 10 * SECOND, Some(1))
      .is_err()
  );
  assert_eq!(
    receiver.install_snapshot(support::signed_snapshot(T_SRV), 10 * SECOND, Some(1)),
    Ok(T_SRV)
  );
  assert!(receiver.on_propose(propose, 20 * SECOND, Some(1)).is_ok());
  // One hour of monotonic time later a cheque with the same expiry has expired.
  assert_eq!(
    receiver.on_propose(pair.propose(2, T_SRV + 3600), 3610 * SECOND, Some(1)),
    Err("cheque expired".to_owned())
  );
}

fn finished_noise_pair() -> (NoiseSession, NoiseSession) {
  let responder_keys = generate_noise_static_keypair().unwrap();
  let mut initiator = NoiseSession::new_initiator(responder_keys.public_key, vec![]).unwrap();
  let mut responder = NoiseSession::new_responder(responder_keys.private_key, vec![]).unwrap();
  let first = initiator.write_message(vec![]).unwrap();
  responder.read_message(first).unwrap();
  let second = responder.write_message(vec![]).unwrap();
  initiator.read_message(second).unwrap();
  let third = initiator.write_message(vec![]).unwrap();
  responder.read_message(third).unwrap();
  (initiator, responder)
}

#[test]
fn noise_pair_key_is_the_same_on_both_ends() {
  let (initiator, responder) = finished_noise_pair();
  let key = initiator.peer_pair_key().unwrap();
  assert_eq!(key.len(), 64);
  assert_eq!(responder.peer_pair_key().unwrap(), key);
}

#[test]
fn bridge_session_caps_one_noise_pair_per_day() {
  const NOW: u64 = 1_900_000_000;
  let directory = tempfile::tempdir().unwrap();
  let buffer = DtnBuffer::open(
    directory
      .path()
      .join("dtn.db")
      .to_string_lossy()
      .into_owned(),
    DB_KEY.to_vec(),
    support::snapshot_public_key(),
    100_000,
  )
  .unwrap();
  let (initiator, _) = finished_noise_pair();
  let pair_key = initiator.peer_pair_key().unwrap();
  let mut counter = 0u32;
  let mut envelope = || {
    counter += 1;
    let batch = (0..4096u32)
      .map(|index| (index.wrapping_mul(31) ^ counter.wrapping_mul(2_654_435_761)) as u8)
      .collect();
    build_dtn_envelope(batch, NOW, 4, 0)
  };
  let mut refused = None;
  'sessions: for _ in 0..40 {
    let session = buffer.open_session(pair_key.clone());
    for _ in 0..20 {
      if let Err(reason) = session.ingest(envelope(), NOW) {
        refused = Some(reason);
        break 'sessions;
      }
    }
  }
  let refused = refused.expect("pair cap never hit");
  assert_eq!(refused, "envelope rejected: Refused");
  let other = buffer.open_session(b"another-pair".to_vec());
  assert_eq!(other.ingest(envelope(), NOW), Ok(true));
}

const DAY: u64 = 86_400;

fn bridge_receiver(
  directory: &std::path::Path,
  expiry_secs: u64,
) -> Result<ReceiverTxEngine, String> {
  ReceiverTxEngine::open(
    directory.join("r.db").to_string_lossy().into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&PAYER_SEED).unwrap()),
    OWN_ACCOUNT.to_vec(),
    support::snapshot_public_key(),
    expiry_secs,
    support::issuer_root_public_key(),
    support::issuer_cert(),
  )
}

#[test]
fn bridge_receiver_rejects_out_of_range_e_at_open() {
  let directory = tempfile::tempdir().unwrap();
  for bad in [24 * 3600 - 1, 168 * 3600 + 1] {
    assert_eq!(
      bridge_receiver(directory.path(), bad).err(),
      Some("expiry E must be 24 to 168 hours".to_owned())
    );
  }
}

#[test]
fn bridge_old_genuine_snapshot_first_does_not_lock_out_the_current_one() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = bridge_receiver(directory.path(), E).unwrap();
  assert_eq!(
    receiver.install_snapshot(support::signed_snapshot(T_SRV), 0, Some(1)),
    Ok(T_SRV)
  );
  let current = T_SRV + 10 * DAY;
  assert_eq!(
    receiver.install_snapshot(support::signed_snapshot(current), SECOND, Some(1)),
    Ok(current)
  );
}

#[test]
fn bridge_snapshot_older_than_persisted_max_is_refused_after_restart() {
  let directory = tempfile::tempdir().unwrap();
  let current = T_SRV + 10 * DAY;
  let receiver = bridge_receiver(directory.path(), E).unwrap();
  receiver
    .install_snapshot(support::signed_snapshot(current), 0, Some(1))
    .unwrap();
  drop(receiver);
  let restarted = bridge_receiver(directory.path(), E).unwrap();
  assert_eq!(
    restarted.install_snapshot(support::signed_snapshot(T_SRV), 0, Some(2)),
    Err("snapshot older than the held one".to_owned())
  );
  // The anchor survived the restart: a fresh cheque is judged without reinstalling.
  let pair = Pair::new();
  assert!(
    restarted
      .on_propose(pair.propose(1, current + 3600), 0, Some(2))
      .is_ok()
  );
}

/// After the protocol clock reached `T_SRV + 2d`, a reboot (or a caller faking one, with a
/// different counter or `None` instead of `Some`) must not roll "now" back to `t_srv`.
#[test]
fn bridge_suspected_reboot_never_rolls_now_back() {
  for rebooted_count in [Some(2), None] {
    let directory = tempfile::tempdir().unwrap();
    let pair = Pair::new();
    let receiver = bridge_receiver(directory.path(), E).unwrap();
    receiver
      .install_snapshot(support::signed_snapshot(T_SRV), 0, Some(1))
      .unwrap();
    assert!(
      receiver
        .on_propose(pair.propose(1, T_SRV + 3 * DAY), 2 * DAY * SECOND, Some(1))
        .is_ok()
    );
    assert_eq!(
      receiver.on_propose(pair.propose(2, T_SRV + DAY), 0, rebooted_count),
      Err("cheque expired".to_owned())
    );
  }
}

#[test]
fn bridge_bogus_monotonic_reading_is_refused_and_not_persisted() {
  let directory = tempfile::tempdir().unwrap();
  let pair = Pair::new();
  let receiver = bridge_receiver(directory.path(), E).unwrap();
  receiver
    .install_snapshot(support::signed_snapshot(T_SRV), 0, Some(1))
    .unwrap();
  assert_eq!(
    receiver.on_propose(pair.propose(1, T_SRV + 3600), u64::MAX / 2, Some(1)),
    Err("revocation snapshot older than S_max".to_owned())
  );
  assert!(
    receiver
      .on_propose(pair.propose(2, T_SRV + 3600), SECOND, Some(1))
      .is_ok()
  );
}

#[test]
fn bridge_snapshot_time_near_i64_max_is_refused() {
  let directory = tempfile::tempdir().unwrap();
  let receiver = bridge_receiver(directory.path(), E).unwrap();
  assert_eq!(
    receiver.install_snapshot(support::signed_snapshot(i64::MAX as u64 - 10), 0, Some(1)),
    Err("snapshot issued_at out of range".to_owned())
  );
}

/// iOS (no boot counter): once a reboot was seen, uptime passing the old anchor must not lift the
/// `expires_at - t_srv <= E` bound before a newer snapshot arrives.
#[test]
fn bridge_ios_reboot_stays_flagged_after_uptime_passes_old_anchor() {
  let directory = tempfile::tempdir().unwrap();
  let pair = Pair::new();
  let receiver = bridge_receiver(directory.path(), E).unwrap();
  receiver
    .install_snapshot(support::signed_snapshot(T_SRV), 100 * SECOND, None)
    .unwrap();
  assert!(
    receiver
      .on_propose(pair.propose(1, T_SRV + 3600), 50 * SECOND, None)
      .is_ok()
  );
  assert_eq!(
    receiver.on_propose(pair.propose(2, T_SRV + E + 3600), 200 * SECOND, None),
    Err("after reboot only cheques expiring within E of t_srv are accepted".to_owned())
  );
}

#[test]
fn stale_clock_row_never_overwrites_a_newer_one() {
  use meshpay_core::receiver_clock_row::ReceiverClockRow;
  let pair = Pair::new();
  let newer = ReceiverClockRow {
    t_srv_unix: T_SRV,
    anchor_unix: T_SRV,
    monotonic_nanos: 0,
    boot_count: Some(1),
    max_now_unix: T_SRV + 200,
    rebooted: true,
  };
  pair.receiver.save_clock(&newer).unwrap();
  pair
    .receiver
    .save_clock(&ReceiverClockRow {
      max_now_unix: T_SRV + 100,
      ..newer
    })
    .unwrap();
  pair
    .receiver
    .save_clock(&ReceiverClockRow {
      rebooted: false,
      ..newer
    })
    .unwrap();
  assert_eq!(pair.receiver.load_clock(), Ok(Some(newer)));
}

#[test]
fn bridge_installed_snapshot_revokes_the_issuer_key() {
  let directory = tempfile::tempdir().unwrap();
  let pair = Pair::new();
  let receiver = ReceiverTxEngine::open(
    directory.path().join("r.db").to_string_lossy().into_owned(),
    DB_KEY.to_vec(),
    PAYEE_SEED.to_vec(),
    compressed_public_key(&signing_key_from_seed(&PAYER_SEED).unwrap()),
    OWN_ACCOUNT.to_vec(),
    support::snapshot_public_key(),
    E,
    support::issuer_root_public_key(),
    support::issuer_cert(),
  )
  .unwrap();
  let snapshot = support::signed_snapshot_revoking(T_SRV, 1, vec![support::issuer_key_id()]);
  assert_eq!(
    receiver.install_snapshot(snapshot, 10 * SECOND, Some(1)),
    Ok(T_SRV)
  );
  assert_eq!(
    receiver.on_propose(pair.propose(1, T_SRV + 3600), 20 * SECOND, Some(1)),
    Err("stub issued by a revoked server key".to_owned())
  );
}
