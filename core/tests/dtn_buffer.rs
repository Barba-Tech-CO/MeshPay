mod support;

use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use meshpay_core::dtn_buffer::{
  BUFFER_CAP_BYTES, DtnEngine, MAX_AGE_SECS, MAX_ENVELOPE_BYTES, MAX_PER_CONNECTION,
  MAX_PER_PAIR_BYTES,
};
use meshpay_core::dtn_ingest_outcome::IngestOutcome;
use meshpay_core::dtn_peer_session::DtnPeerSession;
use meshpay_core::dtn_snapshot::snapshot_signing_message;
use meshpay_core::issuer_trust::IssuerTrust;
use meshpay_core::tx_database::open_database;
use meshpay_core::tx_keys::{sign, signing_key_from_seed};
use meshpay_core::tx_wire::revocation_snapshot::RevocationSnapshot;
use meshpay_core::tx_wire::sync_envelope::SyncEnvelope;
use prost::Message;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const NOW: u64 = 1_900_000_000;
const DB_KEY: [u8; 32] = [5; 32];
const SNAPSHOT_SEED: [u8; 32] = [11; 32];

static SNAPSHOT_TRUST: LazyLock<IssuerTrust> = LazyLock::new(|| {
  IssuerTrust::new_for_role(
    &support::issuer_root_public_key(),
    &support::cert_with(
      [9; 32],
      SNAPSHOT_SEED,
      IssuerTrust::SNAPSHOT_ROLE,
      NOW - 1,
      NOW + 1,
    ),
    IssuerTrust::SNAPSHOT_ROLE,
  )
  .unwrap()
});

fn snapshot_trust() -> &'static IssuerTrust {
  &SNAPSHOT_TRUST
}

const TEST_CAP: i64 = 64 * 1024;
const UNLIMITED_RATE: u32 = 1_000_000;

struct Node {
  _directory: tempfile::TempDir,
  path: PathBuf,
  engine: Arc<DtnEngine>,
}

impl Node {
  fn new() -> Self {
    Self::with_limits(BUFFER_CAP_BYTES, UNLIMITED_RATE)
  }

  fn with_limits(cap_bytes: i64, rate: u32) -> Self {
    Self::with_clock(cap_bytes, rate, Arc::new(AtomicU64::new(0)))
  }

  /// `seconds` is the engine's monotonic clock, advanced by the test.
  fn with_clock(cap_bytes: i64, rate: u32, seconds: Arc<AtomicU64>) -> Self {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("dtn.db");
    let engine = Arc::new(
      DtnEngine::open_with_clock(
        &path,
        &DB_KEY,
        snapshot_trust(),
        cap_bytes,
        rate,
        Box::new(move || Duration::from_secs(seconds.load(Ordering::SeqCst))),
      )
      .unwrap(),
    );
    Self {
      _directory: directory,
      path,
      engine,
    }
  }

  fn session(&self) -> DtnPeerSession {
    DtnPeerSession::new(Arc::clone(&self.engine))
  }

  /// Ids held, read straight from the encrypted file (not capped like `summary`).
  fn held_ids(&self) -> HashSet<Vec<u8>> {
    let connection = open_database(&self.path, &DB_KEY, "").unwrap();
    let mut statement = connection
      .prepare("SELECT envelope_id FROM dtn_envelopes")
      .unwrap();
    statement
      .query_map([], |row| row.get(0))
      .unwrap()
      .collect::<Result<_, _>>()
      .unwrap()
  }

  fn received_at(&self, id: &[u8]) -> i64 {
    open_database(&self.path, &DB_KEY, "")
      .unwrap()
      .query_row(
        "SELECT received_at_unix FROM dtn_envelopes WHERE envelope_id = ?1",
        [id],
        |row| row.get(0),
      )
      .unwrap()
  }
}

fn random_bytes(length: usize) -> Vec<u8> {
  let mut bytes = vec![0u8; length];
  getrandom::fill(&mut bytes).unwrap();
  bytes
}

/// Deterministic 5000-byte payload: SHA-256(counter, block) blocks.
fn payload(counter: u64) -> Vec<u8> {
  (0u32..)
    .flat_map(|block| {
      Sha256::new()
        .chain_update(counter.to_be_bytes())
        .chain_update(block.to_be_bytes())
        .finalize()
        .to_vec()
    })
    .take(5000)
    .collect()
}

fn envelope(payload: Vec<u8>, created_at: u64, ttl_hops: u32) -> Vec<u8> {
  SyncEnvelope::new(payload, created_at, ttl_hops, 0).encode_to_vec()
}

fn id_of(bytes: &[u8]) -> Vec<u8> {
  SyncEnvelope::decode(bytes).unwrap().envelope_id
}

/// One gossip round trip over a fresh connection: `from` offers what `to` lacks.
fn push(from: &Node, to: &Node) -> usize {
  let batch = from
    .engine
    .missing_from(&to.engine.summary().unwrap())
    .unwrap();
  let session = to.session();
  for bytes in &batch {
    session.ingest(bytes, NOW).unwrap();
  }
  batch.len()
}

#[test]
fn pv_dtn_01_carrier_cannot_read_the_payload() {
  let destination_key = random_bytes(32);
  let cipher = ChaCha20Poly1305::new(&Key::try_from(destination_key.as_slice()).unwrap());
  let carrier = Node::new();
  let marker = b"redeem-request-plaintext-marker";
  let mut ids = Vec::new();
  for index in 0..100u32 {
    let mut plaintext = marker.to_vec();
    plaintext.extend_from_slice(&index.to_be_bytes());
    let nonce_bytes = random_bytes(12);
    let mut sealed = nonce_bytes.clone();
    sealed.extend(
      cipher
        .encrypt(
          &Nonce::try_from(&nonce_bytes).unwrap(),
          plaintext.as_slice(),
        )
        .unwrap(),
    );
    let bytes = envelope(sealed, NOW, 4);
    ids.push(SyncEnvelope::decode(bytes.as_slice()).unwrap().envelope_id);
    assert_eq!(
      carrier.engine.ingest(&bytes, NOW).unwrap(),
      IngestOutcome::Stored
    );
  }
  let carried = carrier.engine.envelopes_for(&ids).unwrap();
  assert_eq!(carried.len(), MAX_PER_CONNECTION);
  let carrier_cipher = ChaCha20Poly1305::new(&Key::try_from(DB_KEY.as_slice()).unwrap());
  for bytes in &carried {
    let envelope = SyncEnvelope::decode(bytes.as_slice()).unwrap();
    assert!(envelope.public_proofs.is_empty());
    let (nonce, ciphertext) = envelope.encrypted_server_batch.split_at(12);
    assert!(
      !envelope
        .encrypted_server_batch
        .windows(marker.len())
        .any(|window| window == marker)
    );
    // The carrier holds every key it owns (its database key) and still cannot open it.
    assert!(
      carrier_cipher
        .decrypt(&Nonce::try_from(nonce).unwrap(), ciphertext)
        .is_err()
    );
    assert!(
      cipher
        .decrypt(&Nonce::try_from(nonce).unwrap(), ciphertext)
        .is_ok()
    );
  }
}

/// Offers `count` same-class envelopes (ttl 4) with increasing receive time; checks the cap on
/// every insert and that exactly the newest ones survive (oldest received evicted first).
fn assert_cap_and_oldest_first_eviction(node: &Node, cap: i64, count: u64) {
  let mut sent = Vec::new();
  for counter in 0..count {
    let bytes = envelope(payload(counter), NOW, 4);
    let outcome = node.engine.ingest(&bytes, NOW + counter).unwrap();
    assert_eq!(outcome, IngestOutcome::Stored);
    assert!(node.engine.buffer_bytes().unwrap() <= cap);
    sent.push(id_of(&bytes));
  }
  let held = node.held_ids();
  let kept = held.len();
  assert!(kept > 0 && kept < sent.len());
  let newest: HashSet<Vec<u8>> = sent[sent.len() - kept..].iter().cloned().collect();
  assert_eq!(held, newest, "evicted must be exactly the oldest received");
  // Held ids offered again are duplicates (100%), evicted ones would be stored anew.
  for counter in (count - kept as u64)..count {
    let bytes = envelope(payload(counter), NOW, 4);
    assert_eq!(
      node.engine.ingest(&bytes, NOW + count).unwrap(),
      IngestOutcome::Duplicate
    );
  }
  assert_eq!(node.held_ids(), newest);
}

#[test]
fn pv_dtn_02_small_cap_never_exceeded_dedups_and_evicts_oldest() {
  let node = Node::with_limits(TEST_CAP, UNLIMITED_RATE);
  assert_cap_and_oldest_first_eviction(&node, TEST_CAP, 60);
}

/// PV-DTN-02 as written in the PRD: real 5 MB cap under a 20 MB load (~80 s of fsyncs).
#[test]
#[ignore = "slow: real 5 MB cap, 20 MB load"]
fn pv_dtn_02_real_cap_under_20_mb_load() {
  let node = Node::new();
  assert_cap_and_oldest_first_eviction(&node, BUFFER_CAP_BYTES, 4200);
}

#[test]
fn pv_dtn_02_eviction_order_lowest_ttl_then_oldest_and_refusal_writes_nothing() {
  let node = Node::with_limits(TEST_CAP, UNLIMITED_RATE);
  let low = envelope(payload(1000), NOW, 1);
  let old = envelope(payload(1001), NOW, 3);
  let new = envelope(payload(1002), NOW, 3);
  for (index, bytes) in [&low, &old, &new].into_iter().enumerate() {
    node.engine.ingest(bytes, NOW + index as u64).unwrap();
  }
  let mut counter = 0;
  while node.engine.buffer_bytes().unwrap() + 5100 <= TEST_CAP {
    node
      .engine
      .ingest(&envelope(payload(counter), NOW, 4), NOW + 10 + counter)
      .unwrap();
    counter += 1;
  }
  let (low_id, old_id, new_id) = (id_of(&low), id_of(&old), id_of(&new));
  assert!(
    [&low_id, &old_id, &new_id]
      .iter()
      .all(|id| node.held_ids().contains(*id))
  );
  let mut evicted = Vec::new();
  for step in 0..3 {
    node
      .engine
      .ingest(&envelope(payload(2000 + step), NOW, 4), NOW + 100 + step)
      .unwrap();
    let held = node.held_ids();
    evicted.push([&low_id, &old_id, &new_id].map(|id| !held.contains(id)));
  }
  assert_eq!(
    evicted,
    [
      [true, false, false],
      [true, true, false],
      [true, true, true]
    ]
  );

  // Buffer full of ttl 4: a lower-class newcomer has nothing it may evict and is refused,
  // with the buffer and the database untouched.
  let before = (node.held_ids(), node.engine.buffer_bytes().unwrap());
  for ttl in [0, 1, 3] {
    let outcome = node
      .engine
      .ingest(&envelope(payload(3000 + ttl as u64), NOW, ttl), NOW + 200)
      .unwrap();
    assert_eq!(outcome, IngestOutcome::Refused);
  }
  assert_eq!(
    (node.held_ids(), node.engine.buffer_bytes().unwrap()),
    before
  );
}

#[test]
fn flood_of_ttl_4_with_future_created_at_displaces_at_most_one_session_quota() {
  let seconds = Arc::new(AtomicU64::new(0));
  let node = Node::with_clock(256 * 1024, 30, Arc::clone(&seconds));
  let mut legit = Vec::new();
  for counter in 0..45 {
    seconds.store(counter * 3, Ordering::SeqCst);
    let bytes = envelope(payload(counter), NOW, 2);
    if node.engine.ingest(&bytes, NOW + counter * 3).unwrap() == IngestOutcome::Stored {
      legit.push(id_of(&bytes));
    }
  }
  assert_eq!(legit.len(), 45);
  assert_eq!(node.held_ids().len(), legit.len());

  // One connection, hostile: 100 attempts, ttl 4, created_at as far ahead as allowed.
  let later = NOW + 400;
  seconds.store(400, Ordering::SeqCst);
  let session = node.session();
  let outcomes: Vec<_> = (0..100)
    .map(|counter| {
      session
        .ingest(&envelope(payload(10_000 + counter), later + 600, 4), later)
        .unwrap()
    })
    .collect();
  let accepted = outcomes
    .iter()
    .filter(|o| **o == IngestOutcome::Stored)
    .count();
  assert_eq!(accepted, MAX_PER_CONNECTION);
  assert!(
    outcomes[MAX_PER_CONNECTION..]
      .iter()
      .all(|o| *o == IngestOutcome::SessionBudgetExceeded)
  );
  let held = node.held_ids();
  let displaced = legit.iter().filter(|id| !held.contains(*id)).count();
  assert!(displaced <= MAX_PER_CONNECTION, "displaced {displaced}");
  // Free room (6 envelopes) absorbs part of the 20 accepted ones.
  assert_eq!(displaced, MAX_PER_CONNECTION - 6);

  // Many connections: the global per-minute limit (30) stops the rest.
  let mut stored = accepted;
  let mut limited = false;
  for session_index in 0..5u64 {
    let session = node.session();
    for counter in 0..MAX_PER_CONNECTION as u64 {
      let bytes = envelope(
        payload(20_000 + session_index * 100 + counter),
        later + 600,
        4,
      );
      match session.ingest(&bytes, later).unwrap() {
        IngestOutcome::Stored => stored += 1,
        IngestOutcome::RateLimited => limited = true,
        other => panic!("unexpected {other:?}"),
      }
    }
  }
  assert!(limited);
  assert_eq!(stored, 30);
  let held = node.held_ids();
  assert!(legit.iter().filter(|id| !held.contains(*id)).count() <= 30);
}

#[test]
fn session_budget_counts_rejected_envelopes_too() {
  let node = Node::new();
  let session = node.session();
  for _ in 0..MAX_PER_CONNECTION {
    assert_eq!(
      session.ingest(&[0xff, 0xff], NOW).unwrap(),
      IngestOutcome::Malformed
    );
  }
  let valid = envelope(payload(1), NOW, 4);
  assert_eq!(
    session.ingest(&valid, NOW).unwrap(),
    IngestOutcome::SessionBudgetExceeded
  );
  assert_eq!(node.engine.envelope_count().unwrap(), 0);
  assert_eq!(
    node.session().ingest(&valid, NOW).unwrap(),
    IngestOutcome::Stored
  );
}

#[test]
fn rate_limit_window_slides_on_the_monotonic_clock() {
  let seconds = Arc::new(AtomicU64::new(0));
  let node = Node::with_clock(BUFFER_CAP_BYTES, 3, Arc::clone(&seconds));
  let ingest = |at: u64, counter: u64| {
    seconds.store(at, Ordering::SeqCst);
    node
      .engine
      .ingest(&envelope(payload(counter), NOW, 4), NOW + at)
      .unwrap()
  };
  assert_eq!(ingest(0, 0), IngestOutcome::Stored);
  assert_eq!(ingest(20, 1), IngestOutcome::Stored);
  assert_eq!(ingest(40, 2), IngestOutcome::Stored);
  assert_eq!(ingest(59, 3), IngestOutcome::RateLimited);
  assert_eq!(ingest(60, 3), IngestOutcome::Stored); // the call at 0 slid out
  assert_eq!(ingest(61, 4), IngestOutcome::RateLimited); // 20, 40, 60 still in
  assert_eq!(ingest(80, 4), IngestOutcome::Stored); // 20 slid out
}

#[test]
fn bogus_caller_clock_cannot_wedge_or_bypass_the_limiter() {
  let seconds = Arc::new(AtomicU64::new(0));
  let node = Node::with_clock(BUFFER_CAP_BYTES, 3, Arc::clone(&seconds));
  let bytes = |counter: u64| envelope(payload(counter), NOW, 4);
  assert_eq!(
    node.engine.ingest(&bytes(0), NOW).unwrap(),
    IngestOutcome::Stored
  );
  // Far future, u64::MAX and far past are errors that neither consume budget nor move state.
  for bogus in [NOW + 10 * 86_400, u64::MAX, 0, NOW - 1000] {
    let error = node.engine.ingest(&bytes(1), bogus).unwrap_err();
    assert!(error.contains("caller clock"), "{error}");
  }
  assert_eq!(node.engine.envelope_count().unwrap(), 1);
  // Normal traffic continues; slight backwards drift (within 5 min) is tolerated.
  assert_eq!(
    node.engine.ingest(&bytes(1), NOW + 1000).unwrap(),
    IngestOutcome::Stored
  );
  assert_eq!(
    node.engine.ingest(&bytes(2), NOW + 900).unwrap(),
    IngestOutcome::Stored
  );
  // The window is monotonic: the third accepted call used the last slot, whatever `now` says.
  assert_eq!(
    node.engine.ingest(&bytes(3), NOW + 1000).unwrap(),
    IngestOutcome::RateLimited
  );
}

#[test]
fn repeated_now_zero_is_still_rate_limited() {
  let node = Node::with_limits(BUFFER_CAP_BYTES, 3);
  let outcomes: Vec<_> = (0..6)
    .map(|counter| {
      node
        .engine
        .ingest(&envelope(payload(counter), NOW, 4), 0)
        .unwrap()
    })
    .collect();
  assert_eq!(&outcomes[..3], [IngestOutcome::FromFuture; 3]);
  assert_eq!(&outcomes[3..], [IngestOutcome::RateLimited; 3]);
}

#[test]
fn duplicate_raises_stored_ttl_to_the_max_without_refreshing_received_at() {
  let node = Node::new();
  let low = envelope(payload(7), NOW, 1);
  let id = id_of(&low);
  assert_eq!(
    node.engine.ingest(&low, NOW).unwrap(),
    IngestOutcome::Stored
  );
  let forwarded_hops = |node: &Node| {
    let sent = node
      .engine
      .envelopes_for(std::slice::from_ref(&id))
      .unwrap();
    sent
      .first()
      .map(|bytes| SyncEnvelope::decode(bytes.as_slice()).unwrap().ttl_hops)
  };
  assert_eq!(forwarded_hops(&node), Some(0));
  let high = envelope(payload(7), NOW, 4);
  assert_eq!(
    node.engine.ingest(&high, NOW + 500).unwrap(),
    IngestOutcome::Duplicate
  );
  assert_eq!(forwarded_hops(&node), Some(3));
  // A lower copy never lowers it, and received_at stays the first one.
  assert_eq!(
    node.engine.ingest(&low, NOW + 900).unwrap(),
    IngestOutcome::Duplicate
  );
  assert_eq!(forwarded_hops(&node), Some(3));
  assert_eq!(node.received_at(&id), NOW as i64);
  assert_eq!(node.engine.envelope_count().unwrap(), 1);
}

#[test]
fn pv_dtn_03_envelope_travels_a_to_b_to_c_without_a_direct_link() {
  for _ in 0..10 {
    let (a, b, c) = (Node::new(), Node::new(), Node::new());
    let bytes = envelope(random_bytes(300), NOW, 4);
    let id = SyncEnvelope::decode(bytes.as_slice()).unwrap().envelope_id;
    assert_eq!(a.engine.ingest(&bytes, NOW).unwrap(), IngestOutcome::Stored);
    assert_eq!(push(&a, &b), 1);
    assert_eq!(push(&b, &c), 1);
    assert!(c.engine.summary().unwrap().contains(&id));
    let onward = c.engine.envelopes_for(&[id]).unwrap();
    assert_eq!(
      SyncEnvelope::decode(onward[0].as_slice()).unwrap().ttl_hops,
      1
    );
  }
}

#[test]
fn envelope_with_no_hops_left_is_held_but_not_forwarded() {
  let (a, b, c) = (Node::new(), Node::new(), Node::new());
  a.engine
    .ingest(&envelope(random_bytes(100), NOW, 1), NOW)
    .unwrap();
  assert_eq!(push(&a, &b), 1);
  assert_eq!(b.engine.envelope_count().unwrap(), 1);
  assert_eq!(push(&b, &c), 0);
}

#[test]
fn three_nodes_converge_without_duplicate_storage() {
  let nodes = [Node::new(), Node::new(), Node::new()];
  for node in &nodes {
    for _ in 0..5 {
      node
        .engine
        .ingest(&envelope(random_bytes(200), NOW, 4), NOW)
        .unwrap();
    }
  }
  for _round in 0..10 {
    for (from, to) in [(0, 1), (1, 0), (1, 2), (2, 1), (0, 2), (2, 0)] {
      push(&nodes[from], &nodes[to]);
    }
  }
  let reference = {
    let mut ids = nodes[0].engine.summary().unwrap();
    ids.sort();
    ids
  };
  assert_eq!(reference.len(), 15);
  for node in &nodes {
    let mut ids = node.engine.summary().unwrap();
    ids.sort();
    assert_eq!(ids, reference);
    assert_eq!(node.engine.envelope_count().unwrap(), 15);
  }
  // Nothing left to send once converged.
  assert_eq!(push(&nodes[0], &nodes[1]), 0);
}

#[test]
fn invalid_envelopes_are_rejected_and_never_stored() {
  let node = Node::new();
  let ingest = |bytes: &[u8]| node.engine.ingest(bytes, NOW).unwrap();
  assert_eq!(
    ingest(&envelope(random_bytes(MAX_ENVELOPE_BYTES), NOW, 4)),
    IngestOutcome::Oversize
  );
  assert_eq!(
    ingest(&envelope(random_bytes(50), NOW, 5)),
    IngestOutcome::HopsExceeded
  );
  assert_eq!(
    ingest(&envelope(random_bytes(50), NOW - MAX_AGE_SECS - 1, 4)),
    IngestOutcome::Expired
  );
  assert_eq!(
    ingest(&envelope(random_bytes(50), NOW + 601, 4)),
    IngestOutcome::FromFuture
  );
  let mut forged = SyncEnvelope::new(random_bytes(50), NOW, 4, 0);
  forged.envelope_id = vec![1; 32];
  assert_eq!(ingest(&forged.encode_to_vec()), IngestOutcome::IdMismatch);
  let mut empty = SyncEnvelope::new(Vec::new(), NOW, 4, 0);
  empty.envelope_id = vec![0; 32];
  assert_eq!(ingest(&empty.encode_to_vec()), IngestOutcome::IdMismatch);
  let mut proofs = SyncEnvelope::new(random_bytes(50), NOW, 4, 0);
  proofs.public_proofs.push(vec![1, 2, 3]);
  assert_eq!(
    ingest(&proofs.encode_to_vec()),
    IngestOutcome::UnverifiedProofs
  );
  assert_eq!(ingest(&[0xff, 0xff, 0xff]), IngestOutcome::Malformed);
  assert_eq!(node.engine.envelope_count().unwrap(), 0);

  let good = envelope(random_bytes(50), NOW, 4);
  assert_eq!(ingest(&good), IngestOutcome::Stored);
  assert_eq!(ingest(&good), IngestOutcome::Duplicate);
  assert_eq!(node.engine.envelope_count().unwrap(), 1);
}

#[test]
fn sweep_expires_by_local_received_at_not_wire_created_at() {
  let node = Node::new();
  // Wire timestamp near the old edge of the window; received now.
  let old_wire = envelope(payload(1), NOW - MAX_AGE_SECS + 10, 4);
  let fresh_wire = envelope(payload(2), NOW, 4);
  node.engine.ingest(&old_wire, NOW).unwrap();
  node.engine.ingest(&fresh_wire, NOW + 100).unwrap();
  assert_eq!(node.engine.sweep(NOW + 11 + 1).unwrap(), 0);
  assert_eq!(node.engine.sweep(NOW + MAX_AGE_SECS + 1).unwrap(), 1);
  assert_eq!(node.held_ids(), HashSet::from([id_of(&fresh_wire)]));
}

#[test]
fn data_survives_crash_and_reopen_and_needs_the_key() {
  let node = Node::new();
  let bytes = envelope(random_bytes(100), NOW, 4);
  node.engine.ingest(&bytes, NOW).unwrap();
  let signed = signed_snapshot(7, 3);
  node.engine.ingest_snapshot(&signed).unwrap();
  let Node {
    _directory,
    path,
    engine,
  } = node;
  drop(engine); // no orderly close step exists; WAL + synchronous=FULL carry durability
  let reopened = DtnEngine::open(&path, &DB_KEY, snapshot_trust(), UNLIMITED_RATE).unwrap();
  assert_eq!(reopened.envelope_count().unwrap(), 1);
  assert_eq!(
    reopened.ingest(&bytes, NOW).unwrap(),
    IngestOutcome::Duplicate
  );
  assert_eq!(reopened.snapshot_bytes().unwrap(), Some(signed));
  assert!(DtnEngine::open(&path, &DB_KEY, snapshot_trust(), 0).is_err());
  assert!(DtnEngine::open(&path, &[6; 32], snapshot_trust(), UNLIMITED_RATE).is_err());
}

#[test]
fn random_and_mutated_bytes_never_panic() {
  let node = Node::new();
  let valid = envelope(random_bytes(300), NOW, 4);
  let valid_snapshot = signed_snapshot(1, 5);
  for case in 0..10_000usize {
    let length = usize::from(
      random_bytes(2)
        .iter()
        .fold(0u16, |acc, b| acc << 8 | u16::from(*b)),
    ) % 6000;
    let mut input = random_bytes(length);
    if case % 3 == 1 {
      input = valid.clone();
      let flips = random_bytes(3);
      let index = usize::from(flips[0]) % input.len();
      input[index] ^= flips[1] | 1;
      if flips[2].is_multiple_of(4) {
        input.truncate(index);
      }
    }
    let _ = node.engine.ingest(&input, NOW);
    let _ = node.engine.missing_from(&[input.clone()]);
    let _ = node.engine.envelopes_for(&[input.clone()]);
    if case % 3 == 2 {
      input = valid_snapshot.clone();
      let index = usize::from(random_bytes(1)[0]) % input.len();
      input[index] ^= 0x55;
    }
    // A mutated snapshot must never be accepted: the signature covers every byte.
    assert!(node.engine.ingest_snapshot(&input).is_err());
  }
}

fn signed_snapshot(version: u64, revoked_ids: usize) -> Vec<u8> {
  signed_snapshot_at(version, revoked_ids, NOW)
}

fn signed_snapshot_at(version: u64, revoked_ids: usize, issued_at_unix: u64) -> Vec<u8> {
  let mut snapshot = RevocationSnapshot {
    version,
    issued_at_unix,
    revoked_signer_key_ids: (0..revoked_ids).map(|_| random_bytes(32)).collect(),
    signer_key_id: vec![1; 32],
    ..Default::default()
  };
  let key = signing_key_from_seed(&SNAPSHOT_SEED).unwrap();
  snapshot.server_signature = sign(&key, &snapshot_signing_message(&snapshot));
  snapshot.encode_to_vec()
}

#[test]
fn dtn_engine_rejects_a_validly_signed_snapshot_outside_its_cert_window() {
  let node = Node::new();

  assert!(
    node
      .engine
      .ingest_snapshot(&signed_snapshot_at(5, 2, NOW - 2))
      .unwrap_err()
      .contains("snapshot issued_at outside server key cert window")
  );
  assert_eq!(node.engine.snapshot_bytes().unwrap(), None);
}

#[test]
fn dtn_engine_construction_requires_verified_snapshot_trust() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("verified-trust.db");
  let engine = DtnEngine::open(&path, &DB_KEY, snapshot_trust(), UNLIMITED_RATE).unwrap();
  assert_eq!(engine.ingest_snapshot(&signed_snapshot(5, 2)).unwrap(), 5);

  let invalid_cert = support::cert_with(
    [10; 32],
    SNAPSHOT_SEED,
    IssuerTrust::SNAPSHOT_ROLE,
    NOW - 1,
    NOW + 1,
  );
  assert!(
    IssuerTrust::new_for_role(
      &support::issuer_root_public_key(),
      &invalid_cert,
      IssuerTrust::SNAPSHOT_ROLE,
    )
    .is_err()
  );

  let issuer_trust = IssuerTrust::new(
    &support::issuer_root_public_key(),
    &support::cert_with(
      [9; 32],
      SNAPSHOT_SEED,
      IssuerTrust::ISSUER_ROLE,
      NOW - 1,
      NOW + 1,
    ),
  )
  .unwrap();
  assert!(DtnEngine::open(&path, &DB_KEY, &issuer_trust, UNLIMITED_RATE).is_err());
}

#[test]
fn pv_dtn_04_snapshot_authenticity_and_version_order() {
  let node = Node::new();
  assert_eq!(
    node.engine.ingest_snapshot(&signed_snapshot(5, 2)).unwrap(),
    5
  );

  // Invalid signature: signed by another key.
  let mut forged = RevocationSnapshot::decode(signed_snapshot(9, 2).as_slice()).unwrap();
  let other = signing_key_from_seed(&[12; 32]).unwrap();
  forged.server_signature = sign(&other, &snapshot_signing_message(&forged));
  assert!(
    node
      .engine
      .ingest_snapshot(&forged.encode_to_vec())
      .is_err()
  );
  // Tampered body keeps the old signature.
  let mut tampered = RevocationSnapshot::decode(signed_snapshot(9, 2).as_slice()).unwrap();
  tampered.issuance_frozen = true;
  assert!(
    node
      .engine
      .ingest_snapshot(&tampered.encode_to_vec())
      .is_err()
  );
  // Lower and equal versions are refused even with a valid signature.
  assert!(node.engine.ingest_snapshot(&signed_snapshot(4, 2)).is_err());
  assert!(node.engine.ingest_snapshot(&signed_snapshot(5, 2)).is_err());
  // Higher version with valid signature is accepted.
  assert_eq!(
    node.engine.ingest_snapshot(&signed_snapshot(6, 2)).unwrap(),
    6
  );
  assert_eq!(
    RevocationSnapshot::decode(node.engine.snapshot_bytes().unwrap().unwrap().as_slice())
      .unwrap()
      .version,
    6
  );
}

#[test]
fn signed_snapshot_size_metric_for_the_25_kb_filter_decision() {
  // SYNTHETIC revocation counts (the pilot list does not exist yet); the per-id cost is real.
  let size_for = |ids: usize| signed_snapshot(1, ids).len();
  let per_id = (size_for(1000) - size_for(0)) as f64 / 1000.0;
  let capacity = ((25 * 1024 - size_for(0)) as f64 / per_id) as usize;
  println!(
    "snapshot metric: base={} B, 100 ids={} B, 500 ids={} B, {per_id:.1} B/id, 25 KB holds ~{capacity} revoked ids",
    size_for(0),
    size_for(100),
    size_for(500)
  );
  assert!(size_for(100) < 25 * 1024);
  assert!(size_for(500) < 25 * 1024);
  assert!(size_for(capacity + 50) > 25 * 1024);
}

#[test]
fn snapshot_is_stored_canonically_without_unknown_fields() {
  let node = Node::new();
  let signed = signed_snapshot(8, 2);
  let canonical = signed.clone();
  // Unknown protobuf field 99 (varint) appended: the signature covers the known fields only.
  let mut with_junk = signed;
  with_junk.extend_from_slice(&[0x98, 0x06, 0x2a]);
  assert_ne!(with_junk, canonical);
  assert_eq!(node.engine.ingest_snapshot(&with_junk).unwrap(), 8);
  assert_eq!(node.engine.snapshot_bytes().unwrap(), Some(canonical));
  // The stored canonical form is itself valid, and a later legit snapshot is accepted.
  assert_eq!(
    node.engine.ingest_snapshot(&signed_snapshot(9, 1)).unwrap(),
    9
  );
  // Same version again (the clean copy of the stored one) is a stale duplicate, not an error of state.
  assert!(node.engine.ingest_snapshot(&signed_snapshot(9, 1)).is_err());
}

/// PV-SEC-06: 20 MB of envelopes from one origin pair occupy at most 1 MB; invalid ones are not
/// stored; another pair is still accepted.
#[test]
fn one_origin_pair_holds_at_most_one_megabyte() {
  let node = Node::with_limits(64 * 1024 * 1024, UNLIMITED_RATE);
  let pair = b"payer|payee".to_vec();
  let mut offered = 0usize;
  let mut counter = 0u64;
  while offered < 20 * 1024 * 1024 {
    let session = DtnPeerSession::with_origin_pair(Arc::clone(&node.engine), pair.clone());
    for _ in 0..MAX_PER_CONNECTION {
      let bytes = envelope(payload(counter), NOW, 4);
      counter += 1;
      offered += bytes.len();
      session.ingest(&bytes, NOW).unwrap();
    }
  }
  let invalid = DtnPeerSession::with_origin_pair(Arc::clone(&node.engine), b"x|y".to_vec());
  let mut forged = SyncEnvelope::new(payload(u64::MAX), NOW, 4, 0);
  forged.envelope_id = vec![0; 32];
  assert_eq!(
    invalid.ingest(&forged.encode_to_vec(), NOW).unwrap(),
    IngestOutcome::IdMismatch
  );
  let connection = open_database(&node.path, &DB_KEY, "").unwrap();
  let held: i64 = connection
    .query_row(
      "SELECT COALESCE(SUM(size_bytes), 0) FROM dtn_envelopes WHERE origin_pair = ?1",
      [Sha256::digest(&pair).to_vec()],
      |row| row.get(0),
    )
    .unwrap();
  assert!(held > 0 && held <= MAX_PER_PAIR_BYTES, "held {held}");
  let other = DtnPeerSession::with_origin_pair(Arc::clone(&node.engine), b"other".to_vec());
  assert_eq!(
    other
      .ingest(&envelope(payload(u64::MAX - 1), NOW, 4), NOW)
      .unwrap(),
    IngestOutcome::Stored
  );
}

#[test]
fn database_created_before_origin_pair_is_migrated_on_open() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("dtn.db");
  let old_schema = "
CREATE TABLE dtn_envelopes (
  envelope_id BLOB PRIMARY KEY CHECK (length(envelope_id) = 32),
  created_at_unix INTEGER NOT NULL CHECK (created_at_unix >= 0),
  received_at_unix INTEGER NOT NULL CHECK (received_at_unix >= 0),
  ttl_hops INTEGER NOT NULL CHECK (ttl_hops BETWEEN 0 AND 4),
  known_revocation_version INTEGER NOT NULL CHECK (known_revocation_version >= 0),
  encrypted_server_batch BLOB NOT NULL CHECK (length(encrypted_server_batch) > 0),
  size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 5120)
) STRICT;";
  drop(open_database(&path, &DB_KEY, old_schema).unwrap());
  let engine = DtnEngine::open(&path, &DB_KEY, snapshot_trust(), UNLIMITED_RATE).unwrap();
  let session = DtnPeerSession::with_origin_pair(Arc::new(engine), b"a|b".to_vec());
  assert_eq!(
    session.ingest(&envelope(payload(1), NOW, 4), NOW).unwrap(),
    IngestOutcome::Stored
  );
}

#[test]
fn huge_origin_pair_key_does_not_bloat_the_row() {
  let node = Node::new();
  let huge = vec![7u8; 1024 * 1024];
  let session = DtnPeerSession::with_origin_pair(Arc::clone(&node.engine), huge);
  assert_eq!(
    session.ingest(&envelope(payload(2), NOW, 4), NOW).unwrap(),
    IngestOutcome::Stored
  );
  let connection = open_database(&node.path, &DB_KEY, "").unwrap();
  let length: i64 = connection
    .query_row("SELECT length(origin_pair) FROM dtn_envelopes", [], |row| {
      row.get(0)
    })
    .unwrap();
  assert_eq!(length, 32);
}
