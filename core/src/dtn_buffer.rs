use crate::dtn_ingest_outcome::IngestOutcome;
use crate::dtn_snapshot::verify_snapshot;
use crate::issuer_trust::IssuerTrust;
use crate::tx_database::{open_database, sql_error, to_sql_integer};
use crate::tx_wire::sync_envelope::SyncEnvelope;
use p256::ecdsa::VerifyingKey;
use prost::Message;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// PRD 8.1 limits.
pub const MAX_ENVELOPE_BYTES: usize = 5 * 1024;
pub const BUFFER_CAP_BYTES: i64 = 5 * 1024 * 1024;
/// Envelopes per connection, both directions: sent per call and accepted-or-rejected per session.
/// PRD 8.1 DoS: one origin pair holds at most 1 MB of the buffer per day.
pub const MAX_PER_PAIR_BYTES: i64 = 1024 * 1024;
pub const MAX_PER_CONNECTION: usize = 20;
/// Upper bound of `E` (24 to 168 h, PRD 9; free choice, no source). Applied to the local
/// `received_at_unix`, never to the wire `created_at_unix`.
pub const MAX_AGE_SECS: u64 = 168 * 3600;
/// Plain id list exchanged as summary (PRD 8.1 defers Bloom/Cuckoo). 256 x 32 B = 8 KB.
pub const MAX_SUMMARY_IDS: usize = 256;

fn schema() -> &'static str {
  "
CREATE TABLE IF NOT EXISTS dtn_envelopes (
  envelope_id BLOB PRIMARY KEY CHECK (length(envelope_id) = 32),
  created_at_unix INTEGER NOT NULL CHECK (created_at_unix >= 0),
  received_at_unix INTEGER NOT NULL CHECK (received_at_unix >= 0),
  ttl_hops INTEGER NOT NULL CHECK (ttl_hops BETWEEN 0 AND 4),
  known_revocation_version INTEGER NOT NULL CHECK (known_revocation_version >= 0),
  encrypted_server_batch BLOB NOT NULL CHECK (length(encrypted_server_batch) > 0),
  size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 5120),
  origin_pair BLOB
) STRICT;
CREATE INDEX IF NOT EXISTS dtn_envelopes_eviction ON dtn_envelopes (ttl_hops, received_at_unix);
CREATE TABLE IF NOT EXISTS dtn_snapshot (
  slot INTEGER PRIMARY KEY CHECK (slot = 1),
  version INTEGER NOT NULL CHECK (version >= 0),
  issued_at_unix INTEGER NOT NULL CHECK (issued_at_unix >= 0),
  snapshot_bytes BLOB NOT NULL
) STRICT;"
}

/// Databases created before `origin_pair` lack the column; add it, then index it.
fn migrate_origin_pair(connection: &Connection) -> Result<(), String> {
  let has_column: bool = connection
    .query_row(
      "SELECT COUNT(*) > 0 FROM pragma_table_info('dtn_envelopes') WHERE name = 'origin_pair'",
      [],
      |row| row.get(0),
    )
    .map_err(sql_error)?;
  if !has_column {
    connection
      .execute_batch("ALTER TABLE dtn_envelopes ADD COLUMN origin_pair BLOB")
      .map_err(sql_error)?;
  }
  connection
    .execute_batch(
      "CREATE INDEX IF NOT EXISTS dtn_envelopes_pair ON dtn_envelopes (origin_pair, received_at_unix)",
    )
    .map_err(sql_error)
}

/// Third-party envelope buffer and gossip exchange logic (PRD 8.1), foreground only.
///
/// Eviction policy (stored `received_at_unix` is local; wire `created_at_unix` is only checked
/// against the window at ingest): when full, a new envelope may evict only held envelopes whose
/// `ttl_hops` is the same or lower, lowest `ttl_hops` first, then oldest received. If those do
/// not free enough room the new envelope is refused before anything is written. A flood of
/// `ttl_hops = 4` therefore can evict legitimate envelopes, but each accepted insert frees at
/// most its own size (<= 5 KB) and a `DtnPeerSession` accepts 20 envelopes, so one connection
/// displaces at most 20 envelopes / 100 KB; the global per-minute limit bounds many connections.
pub struct DtnEngine {
  connection: Mutex<Connection>,
  snapshot_key: VerifyingKey,
  snapshot_cert_window: (u64, u64),
  cap_bytes: i64,
  max_ingest_per_minute: usize,
  /// Monotonic time since engine start; the rate limiter never reads the caller's clock.
  clock: Box<dyn Fn() -> Duration + Send + Sync>,
  recent_ingests: Mutex<VecDeque<Duration>>,
  last_seen_unix: Mutex<Option<u64>>,
}

impl DtnEngine {
  /// `db_key` is the raw 32-byte SQLCipher key; `snapshot_trust` must be a verified SNAPSHOT cert.
  pub fn open(
    db_path: &Path,
    db_key: &[u8],
    snapshot_trust: &IssuerTrust,
    max_ingest_per_minute: u32,
  ) -> Result<Self, String> {
    Self::open_with_cap(
      db_path,
      db_key,
      snapshot_trust,
      BUFFER_CAP_BYTES,
      max_ingest_per_minute,
    )
  }

  pub fn open_with_cap(
    db_path: &Path,
    db_key: &[u8],
    snapshot_trust: &IssuerTrust,
    cap_bytes: i64,
    max_ingest_per_minute: u32,
  ) -> Result<Self, String> {
    let start = Instant::now();
    Self::open_with_clock(
      db_path,
      db_key,
      snapshot_trust,
      cap_bytes,
      max_ingest_per_minute,
      Box::new(move || start.elapsed()),
    )
  }

  /// `clock` returns monotonic elapsed time; injectable so tests are deterministic.
  pub fn open_with_clock(
    db_path: &Path,
    db_key: &[u8],
    snapshot_trust: &IssuerTrust,
    cap_bytes: i64,
    max_ingest_per_minute: u32,
    clock: Box<dyn Fn() -> Duration + Send + Sync>,
  ) -> Result<Self, String> {
    if cap_bytes <= 0 || max_ingest_per_minute == 0 {
      return Err("cap and ingest rate limit must be positive".to_owned());
    }
    snapshot_trust.require_role(IssuerTrust::SNAPSHOT_ROLE)?;
    let connection = open_database(db_path, db_key, schema())?;
    migrate_origin_pair(&connection)?;
    Ok(Self {
      connection: Mutex::new(connection),
      snapshot_key: *snapshot_trust.public_key(),
      snapshot_cert_window: snapshot_trust.cert_window(),
      cap_bytes,
      max_ingest_per_minute: max_ingest_per_minute as usize,
      clock,
      recent_ingests: Mutex::new(VecDeque::new()),
      last_seen_unix: Mutex::new(None),
    })
  }

  /// Rate limit (global), size, shape, id, hops and window checks run before the database is
  /// touched. A duplicate only raises the stored `ttl_hops` (never `received_at`). No per-connection
  /// budget here: peer traffic must go through `DtnPeerSession`. Never panics on bytes.
  pub fn ingest(&self, envelope_bytes: &[u8], now_unix: u64) -> Result<IngestOutcome, String> {
    self.ingest_from(envelope_bytes, now_unix, None)
  }

  /// `None` is uncapped: callers must pass a pair key derived from the authenticated session
  /// (never from peer-supplied bytes). The key is hashed to 32 bytes before storing or counting.
  /// `origin_pair` is an opaque key of the (sender, receiver) pair of the connection the envelope
  /// came over (the wire `SyncEnvelope` carries none); `Some` caps that pair at
  /// `MAX_PER_PAIR_BYTES` received in the last day, refusing instead of evicting others.
  pub fn ingest_from(
    &self,
    envelope_bytes: &[u8],
    now_unix: u64,
    origin_pair: Option<&[u8]>,
  ) -> Result<IngestOutcome, String> {
    let origin_pair = origin_pair.map(|pair| Sha256::digest(pair).to_vec());
    self.check_caller_clock(now_unix)?;
    if !self.admit()? {
      return Ok(IngestOutcome::RateLimited);
    }
    if envelope_bytes.len() > MAX_ENVELOPE_BYTES {
      return Ok(IngestOutcome::Oversize);
    }
    let Ok(envelope) = SyncEnvelope::decode(envelope_bytes) else {
      return Ok(IngestOutcome::Malformed);
    };
    if let Some(rejection) = rejection(&envelope, now_unix) {
      return Ok(rejection);
    }
    let size = i64::try_from(envelope.encoded_len()).map_err(|_| "size".to_owned())?;
    let mut connection = self.connection()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(sql_error)?;
    let stored_hops: Option<u32> = transaction
      .query_row(
        "SELECT ttl_hops FROM dtn_envelopes WHERE envelope_id = ?1",
        [&envelope.envelope_id],
        |row| row.get(0),
      )
      .optional()
      .map_err(sql_error)?;
    if let Some(stored_hops) = stored_hops {
      if envelope.ttl_hops > stored_hops {
        transaction
          .execute(
            "UPDATE dtn_envelopes SET ttl_hops = ?2 WHERE envelope_id = ?1",
            params![envelope.envelope_id, envelope.ttl_hops],
          )
          .map_err(sql_error)?;
      }
      transaction.commit().map_err(sql_error)?;
      return Ok(IngestOutcome::Duplicate);
    }
    if let Some(pair) = &origin_pair {
      let held: i64 = transaction
        .query_row(
          "SELECT COALESCE(SUM(size_bytes), 0) FROM dtn_envelopes
           WHERE origin_pair = ?1 AND received_at_unix > ?2",
          params![pair, to_sql_integer(now_unix.saturating_sub(86_400))?],
          |row| row.get(0),
        )
        .map_err(sql_error)?;
      if held + size > MAX_PER_PAIR_BYTES {
        return Ok(IngestOutcome::Refused);
      }
    }
    let needed = total_bytes(&transaction)? + size - self.cap_bytes;
    if needed > 0 {
      let Some(victims) = victims(&transaction, envelope.ttl_hops, needed)? else {
        return Ok(IngestOutcome::Refused);
      };
      for victim in victims {
        transaction
          .execute("DELETE FROM dtn_envelopes WHERE envelope_id = ?1", [victim])
          .map_err(sql_error)?;
      }
    }
    transaction
      .execute(
        "INSERT INTO dtn_envelopes
           (envelope_id, created_at_unix, received_at_unix, ttl_hops, known_revocation_version,
            encrypted_server_batch, size_bytes, origin_pair)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
          envelope.envelope_id,
          to_sql_integer(envelope.created_at_unix)?,
          to_sql_integer(now_unix)?,
          envelope.ttl_hops,
          to_sql_integer(envelope.known_revocation_version)?,
          envelope.encrypted_server_batch,
          size,
          origin_pair
        ],
      )
      .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(IngestOutcome::Stored)
  }

  /// The caller's `now_unix` only feeds the creation window and `received_at`, so it is kept
  /// sane: no more than 5 minutes behind, nor more than 1 day ahead of, the newest accepted
  /// value. The first call is trusted (nothing to compare with).
  fn check_caller_clock(&self, now_unix: u64) -> Result<(), String> {
    let mut last_seen = self
      .last_seen_unix
      .lock()
      .map_err(|_| "clock lock poisoned".to_owned())?;
    if let Some(last) = *last_seen {
      if now_unix.saturating_add(300) < last {
        return Err("caller clock went backwards".to_owned());
      }
      if now_unix > last.saturating_add(86_400) {
        return Err("caller clock jumped more than a day ahead".to_owned());
      }
    }
    *last_seen = Some(last_seen.map_or(now_unix, |last| last.max(now_unix)));
    Ok(())
  }

  /// Sliding one-minute window on the monotonic clock.
  fn admit(&self) -> Result<bool, String> {
    let now = (self.clock)();
    let mut recent = self
      .recent_ingests
      .lock()
      .map_err(|_| "rate limiter lock poisoned".to_owned())?;
    while recent
      .front()
      .is_some_and(|at| *at + Duration::from_secs(60) <= now)
    {
      recent.pop_front();
    }
    if recent.len() >= self.max_ingest_per_minute {
      return Ok(false);
    }
    recent.push_back(now);
    Ok(true)
  }

  /// Ids of every held envelope, best first (highest `ttl_hops`, then newest), capped.
  pub fn summary(&self) -> Result<Vec<Vec<u8>>, String> {
    let connection = self.connection()?;
    let mut statement = connection
      .prepare(
        "SELECT envelope_id FROM dtn_envelopes
         ORDER BY ttl_hops DESC, received_at_unix DESC, envelope_id LIMIT ?1",
      )
      .map_err(sql_error)?;
    statement
      .query_map([MAX_SUMMARY_IDS as i64], |row| row.get(0))
      .map_err(sql_error)?
      .collect::<Result<_, _>>()
      .map_err(sql_error)
  }

  /// Forwardable envelopes (bytes, `ttl_hops` already decremented) the peer's summary lacks,
  /// at most `MAX_PER_CONNECTION`.
  pub fn missing_from(&self, peer_summary: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
    let known: HashSet<&[u8]> = peer_summary
      .iter()
      .take(MAX_SUMMARY_IDS)
      .map(Vec::as_slice)
      .collect();
    let connection = self.connection()?;
    let mut statement = connection
      .prepare(&format!(
        "SELECT {COLUMNS} FROM dtn_envelopes WHERE ttl_hops > 0
         ORDER BY ttl_hops DESC, received_at_unix DESC, envelope_id"
      ))
      .map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    let mut out = Vec::new();
    while out.len() < MAX_PER_CONNECTION {
      let Some(row) = rows.next().map_err(sql_error)? else {
        break;
      };
      let envelope = read_envelope(row)?;
      if !known.contains(envelope.envelope_id.as_slice()) {
        out.push(forwarded(envelope));
      }
    }
    Ok(out)
  }

  /// Forwardable envelopes for the requested ids (unknown ids and ids with no hops left are
  /// skipped), at most `MAX_PER_CONNECTION`.
  pub fn envelopes_for(&self, ids: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
    let connection = self.connection()?;
    let mut statement = connection
      .prepare(&format!(
        "SELECT {COLUMNS} FROM dtn_envelopes WHERE envelope_id = ?1 AND ttl_hops > 0"
      ))
      .map_err(sql_error)?;
    let mut out = Vec::new();
    for id in ids
      .iter()
      .take(MAX_PER_CONNECTION)
      .filter(|id| id.len() == 32)
    {
      let envelope = statement
        .query_row([id], |row| Ok(read_envelope(row)))
        .optional()
        .map_err(sql_error)?
        .transpose()?;
      out.extend(envelope.map(forwarded));
    }
    Ok(out)
  }

  /// Deletes envelopes received more than `MAX_AGE_SECS` ago; returns how many.
  pub fn sweep(&self, now_unix: u64) -> Result<u32, String> {
    let cutoff = to_sql_integer(now_unix.saturating_sub(MAX_AGE_SECS))?;
    let deleted = self
      .connection()?
      .execute(
        "DELETE FROM dtn_envelopes WHERE received_at_unix < ?1",
        [cutoff],
      )
      .map_err(sql_error)?;
    u32::try_from(deleted).map_err(|_| "count".to_owned())
  }

  pub fn buffer_bytes(&self) -> Result<i64, String> {
    total_bytes(&*self.connection()?)
  }

  pub fn envelope_count(&self) -> Result<i64, String> {
    self
      .connection()?
      .query_row("SELECT count(*) FROM dtn_envelopes", [], |row| row.get(0))
      .map_err(sql_error)
  }

  /// Stores a signed snapshot only when the SNAPSHOT-key signature is valid and its version is
  /// strictly greater than the stored one (PV-DTN-04). Returns the accepted version.
  pub fn ingest_snapshot(&self, snapshot_bytes: &[u8]) -> Result<u64, String> {
    let snapshot = verify_snapshot(snapshot_bytes, &self.snapshot_key)?;
    IssuerTrust::check_snapshot_time(snapshot.issued_at_unix, self.snapshot_cert_window)?;
    let mut connection = self.connection()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(sql_error)?;
    let current: Option<i64> = transaction
      .query_row(
        "SELECT version FROM dtn_snapshot WHERE slot = 1",
        [],
        |row| row.get(0),
      )
      .optional()
      .map_err(sql_error)?;
    let version = to_sql_integer(snapshot.version)?;
    if current.is_some_and(|current| version <= current) {
      return Err("snapshot version not newer than the stored one".to_owned());
    }
    transaction
      .execute(
        "INSERT OR REPLACE INTO dtn_snapshot (slot, version, issued_at_unix, snapshot_bytes)
         VALUES (1, ?1, ?2, ?3)",
        params![
          version,
          to_sql_integer(snapshot.issued_at_unix)?,
          snapshot.encode_to_vec()
        ],
      )
      .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(snapshot.version)
  }

  /// Stored snapshot bytes (to forward to peers), if any.
  pub fn snapshot_bytes(&self) -> Result<Option<Vec<u8>>, String> {
    self
      .connection()?
      .query_row(
        "SELECT snapshot_bytes FROM dtn_snapshot WHERE slot = 1",
        [],
        |row| row.get(0),
      )
      .optional()
      .map_err(sql_error)
  }

  fn connection(&self) -> Result<MutexGuard<'_, Connection>, String> {
    self
      .connection
      .lock()
      .map_err(|_| "database lock poisoned".to_owned())
  }
}

const COLUMNS: &str =
  "envelope_id, created_at_unix, ttl_hops, known_revocation_version, encrypted_server_batch";

fn rejection(envelope: &SyncEnvelope, now_unix: u64) -> Option<IngestOutcome> {
  if !envelope.public_proofs.is_empty() {
    return Some(IngestOutcome::UnverifiedProofs);
  }
  if envelope.encrypted_server_batch.is_empty()
    || envelope.envelope_id != Sha256::digest(&envelope.encrypted_server_batch).as_slice()
  {
    return Some(IngestOutcome::IdMismatch);
  }
  if envelope.ttl_hops > 4 {
    return Some(IngestOutcome::HopsExceeded);
  }
  if envelope.created_at_unix < now_unix.saturating_sub(MAX_AGE_SECS) {
    return Some(IngestOutcome::Expired);
  }
  if envelope.created_at_unix > now_unix.saturating_add(600) {
    return Some(IngestOutcome::FromFuture);
  }
  None
}

fn total_bytes(connection: &Connection) -> Result<i64, String> {
  connection
    .query_row(
      "SELECT COALESCE(SUM(size_bytes), 0) FROM dtn_envelopes",
      [],
      |row| row.get(0),
    )
    .map_err(sql_error)
}

/// Ids to delete to free `needed` bytes for an envelope with `new_hops`, or `None` if the
/// eligible envelopes (same or lower `ttl_hops`) do not hold enough.
fn victims(
  connection: &Connection,
  new_hops: u32,
  needed: i64,
) -> Result<Option<Vec<Vec<u8>>>, String> {
  let mut statement = connection
    .prepare(
      "SELECT envelope_id, size_bytes FROM dtn_envelopes WHERE ttl_hops <= ?1
       ORDER BY ttl_hops, received_at_unix, envelope_id",
    )
    .map_err(sql_error)?;
  let mut rows = statement.query([new_hops]).map_err(sql_error)?;
  let (mut freed, mut ids) = (0i64, Vec::new());
  while freed < needed {
    let Some(row) = rows.next().map_err(sql_error)? else {
      return Ok(None);
    };
    ids.push(row.get(0).map_err(sql_error)?);
    freed += row.get::<_, i64>(1).map_err(sql_error)?;
  }
  Ok(Some(ids))
}

fn read_envelope(row: &rusqlite::Row<'_>) -> Result<SyncEnvelope, String> {
  let created_at: i64 = row.get(1).map_err(sql_error)?;
  let known_revocation_version: i64 = row.get(3).map_err(sql_error)?;
  Ok(SyncEnvelope {
    envelope_id: row.get(0).map_err(sql_error)?,
    created_at_unix: u64::try_from(created_at).map_err(|_| "created_at".to_owned())?,
    ttl_hops: row.get(2).map_err(sql_error)?,
    known_revocation_version: u64::try_from(known_revocation_version)
      .map_err(|_| "revocation version".to_owned())?,
    public_proofs: Vec::new(),
    encrypted_server_batch: row.get(4).map_err(sql_error)?,
  })
}

/// Wire bytes for the next hop: one hop consumed.
fn forwarded(envelope: SyncEnvelope) -> Vec<u8> {
  SyncEnvelope {
    ttl_hops: envelope.ttl_hops.saturating_sub(1),
    ..envelope
  }
  .encode_to_vec()
}
