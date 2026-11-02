use meshpay_core::api::{PayerTxEngine, ReceiverTxEngine, generate_lab_signing_key};
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_hashing::{derive_tx_id, spend_hash, spend_signing_message};
use meshpay_core::tx_keys::{compressed_public_key, signing_key_from_seed};
use meshpay_core::tx_wire::ack_receipt::AckReceipt;
use meshpay_core::tx_wire::cheque_spend::ChequeSpend;
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::tx_propose::TxPropose;
use prost::Message;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const PAYER_SEED: [u8; 32] = [7; 32];
const PAYEE_SEED: [u8; 32] = [9; 32];
const DB_KEY: [u8; 32] = [3; 32];
const OWN_ACCOUNT: &[u8] = b"payee-account";

struct Lab {
  directory: tempfile::TempDir,
  payer_public_key: Vec<u8>,
  payee_public_key: Vec<u8>,
}

impl Lab {
  fn new() -> Self {
    Self {
      directory: tempfile::tempdir().expect("tempdir"),
      payer_public_key: compressed_public_key(&signing_key_from_seed(&PAYER_SEED).unwrap()),
      payee_public_key: compressed_public_key(&signing_key_from_seed(&PAYEE_SEED).unwrap()),
    }
  }

  fn payer_path(&self) -> PathBuf {
    self.directory.path().join("payer.db")
  }

  fn receiver_path(&self) -> PathBuf {
    self.directory.path().join("receiver.db")
  }

  /// Opening the file again is the "crash": the old engine was dropped, state is whatever is on disk.
  fn open_payer(&self) -> PayerEngine {
    PayerEngine::open(
      &self.payer_path(),
      &DB_KEY,
      &PAYER_SEED,
      &self.payee_public_key,
    )
    .expect("open payer")
  }

  fn open_receiver(&self) -> ReceiverEngine {
    ReceiverEngine::open(
      &self.receiver_path(),
      &DB_KEY,
      &PAYEE_SEED,
      &self.payer_public_key,
      OWN_ACCOUNT,
    )
    .expect("open receiver")
  }
}

fn spend_body(index: u64) -> (Vec<u8>, Vec<u8>) {
  let payee = OWN_ACCOUNT.to_vec();
  let body = ChequeSpendBody {
    cheque_id: format!("cheque-{index:08}").into_bytes(),
    payee_account_id: payee.clone(),
    amount_units: 100 + index,
    counter: index + 1,
    signed_at_unix: 1_700_000_000 + index,
    ..Default::default()
  };
  (body.encode_to_vec(), payee)
}

fn query_i64(path: &Path, sql: &str) -> i64 {
  let connection = Connection::open(path).expect("open raw");
  let hex_key: String = DB_KEY.iter().map(|b| format!("{b:02x}")).collect();
  connection
    .pragma_update(None, "key", format!("x'{hex_key}'"))
    .expect("key");
  connection
    .query_row(sql, [], |row| row.get(0))
    .expect("query")
}

fn payer_states(lab: &Lab) -> (i64, i64, i64) {
  let path = lab.payer_path();
  (
    query_i64(&path, "SELECT count(*) FROM outgoing_cheques"),
    query_i64(
      &path,
      "SELECT count(*) FROM outgoing_cheques WHERE state = 'LOCKED_OUTGOING'",
    ),
    query_i64(
      &path,
      "SELECT count(*) FROM outgoing_cheques WHERE state = 'COMMITTED'",
    ),
  )
}

fn tx_id_of(propose: &[u8]) -> Vec<u8> {
  let spend = TxPropose::decode(propose).unwrap().spend.unwrap();
  ChequeSpendBody::decode(spend.body.as_slice())
    .unwrap()
    .tx_id
}

/// PV-BLE-03: 200 executions, a crash (engine dropped, DB file reopened) at each failure point.
#[test]
fn pv_ble_03_atomic_failure_recovery_over_200_executions() {
  for index in 0..200u64 {
    let lab = Lab::new();
    let (body, payee) = spend_body(index);

    let payer = lab.open_payer();
    let propose = payer.propose(&body, index + 1, &payee).expect("propose");
    let tx_id = tx_id_of(&propose);

    let (committed_ack, ack_held_by_payer): (Vec<u8>, Option<Vec<u8>>) = match index % 4 {
      // Point 0: crash before anything is sent.
      0 => {
        drop(payer);
        let payer = lab.open_payer();
        assert_eq!(
          payer_states(&lab),
          (1, 1, 0),
          "run {index}: cheque lost before send"
        );
        let pending = payer.resume_pending().unwrap();
        assert_eq!(
          pending,
          vec![propose.clone()],
          "run {index}: redelivery differs"
        );
        assert_eq!(
          payer.propose(&body, index + 1, &payee).unwrap(),
          propose,
          "run {index}: re-propose produced different bytes"
        );
        let ack = lab.open_receiver().on_propose(&propose).expect("receiver");
        (ack, None)
      }
      // Point 1: sent, receiver acked, ack lost; payer crashed meanwhile.
      1 => {
        let first_ack = lab.open_receiver().on_propose(&propose).expect("receiver");
        drop(payer);
        let payer = lab.open_payer();
        assert_eq!(
          payer_states(&lab),
          (1, 1, 0),
          "run {index}: not locked after crash"
        );
        let pending = payer.resume_pending().unwrap();
        assert_eq!(pending, vec![propose.clone()]);
        let redelivered_ack = lab
          .open_receiver()
          .on_propose(&pending[0])
          .expect("redeliver");
        assert_eq!(
          redelivered_ack, first_ack,
          "run {index}: ack not idempotent"
        );
        (first_ack, None)
      }
      // Point 2: payer holds the ack, crashes before persisting COMMITTED.
      2 => {
        let ack = lab.open_receiver().on_propose(&propose).expect("receiver");
        drop(payer);
        assert_eq!(
          payer_states(&lab),
          (1, 1, 0),
          "run {index}: committed without on_ack"
        );
        (ack.clone(), Some(ack))
      }
      // Point 3: receiver crashes after storing, before the ack exists.
      _ => {
        let receiver = lab.open_receiver();
        receiver.store_propose(&propose).expect("store");
        drop(receiver);
        drop(payer);
        let receiver = lab.open_receiver();
        assert_eq!(
          query_i64(
            &lab.receiver_path(),
            "SELECT count(*) FROM incoming_cheques WHERE state = 'RECEIVED'"
          ),
          1,
          "run {index}: receiver lost the stored cheque"
        );
        let payer = lab.open_payer();
        let pending = payer.resume_pending().unwrap();
        assert_eq!(pending, vec![propose.clone()]);
        let ack = receiver.on_propose(&pending[0]).expect("redeliver");
        (ack, None)
      }
    };

    // Reconnect: payer (reopened) commits, with the held ack or the redelivered one.
    let payer = lab.open_payer();
    payer
      .on_ack(ack_held_by_payer.as_deref().unwrap_or(&committed_ack))
      .expect("on_ack");
    assert_eq!(
      payer_states(&lab),
      (1, 0, 1),
      "run {index}: payer not committed"
    );
    payer.on_ack(&committed_ack).expect("on_ack is idempotent");
    assert!(payer.resume_pending().unwrap().is_empty());

    // Late duplicate delivery of the same propose: same ack bytes, no second row.
    let receiver = lab.open_receiver();
    let late_ack = receiver.on_propose(&propose).expect("late redelivery");
    assert_eq!(
      late_ack, committed_ack,
      "run {index}: ack redelivery not byte-identical"
    );
    let receiver_path = lab.receiver_path();
    assert_eq!(
      query_i64(&receiver_path, "SELECT count(*) FROM incoming_cheques"),
      1
    );
    assert_eq!(
      query_i64(
        &receiver_path,
        "SELECT count(*) FROM incoming_cheques WHERE state = 'ACKED'"
      ),
      1
    );
    assert_eq!(
      query_i64(
        &receiver_path,
        &format!(
          "SELECT count(*) FROM incoming_cheques WHERE hex(tx_id) = '{}'",
          tx_id.iter().map(|b| format!("{b:02X}")).collect::<String>()
        )
      ),
      1,
      "run {index}: duplicated incoming cheque"
    );
  }
}

#[test]
fn same_counter_with_different_content_is_refused_and_changes_nothing() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let (body, payee) = spend_body(0);
  let first = payer.propose(&body, 1, &payee).unwrap();

  let other = ChequeSpendBody {
    amount_units: 999,
    ..ChequeSpendBody::decode(body.as_slice()).unwrap()
  };
  assert!(payer.propose(&other.encode_to_vec(), 1, &payee).is_err());
  let other_cheque = ChequeSpendBody {
    cheque_id: b"another-cheque".to_vec(),
    ..ChequeSpendBody::decode(body.as_slice()).unwrap()
  };
  assert!(
    payer
      .propose(&other_cheque.encode_to_vec(), 1, &payee)
      .is_err()
  );
  assert_eq!(payer.resume_pending().unwrap(), vec![first]);
  assert_eq!(payer_states(&lab), (1, 1, 0));
}

#[test]
fn propose_rejects_mismatched_counter_recipient_and_tx_id() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let (body, payee) = spend_body(0);
  assert!(payer.propose(&body, 2, &payee).is_err());
  assert!(payer.propose(&body, 1, b"someone-else").is_err());
  let forged = ChequeSpendBody {
    tx_id: vec![1; 32],
    ..ChequeSpendBody::decode(body.as_slice()).unwrap()
  };
  assert!(payer.propose(&forged.encode_to_vec(), 1, &payee).is_err());
  assert_eq!(payer_states(&lab).0, 0);
}

#[test]
fn receiver_rejects_tampered_body_bad_signature_and_conflicting_content() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();
  let (body, payee) = spend_body(0);
  let propose = payer.propose(&body, 1, &payee).unwrap();
  let spend = TxPropose::decode(propose.as_slice())
    .unwrap()
    .spend
    .unwrap();

  let rewrap = |spend: ChequeSpend| TxPropose { spend: Some(spend) }.encode_to_vec();

  // Body changed after signing: signature check fails.
  let mut tampered = ChequeSpendBody::decode(spend.body.as_slice()).unwrap();
  tampered.amount_units += 1;
  assert!(
    receiver
      .on_propose(&rewrap(ChequeSpend {
        body: tampered.encode_to_vec(),
        ..spend.clone()
      }))
      .is_err()
  );

  // Properly signed by the payer key but tx_id not derived from counter and content.
  let payer_key = signing_key_from_seed(&PAYER_SEED).unwrap();
  let mut wrong_tx_id = ChequeSpendBody::decode(spend.body.as_slice()).unwrap();
  wrong_tx_id.tx_id = vec![5; 32];
  let wrong_body = wrong_tx_id.encode_to_vec();
  let signature = meshpay_core::tx_keys::sign(&payer_key, &spend_signing_message(&wrong_body));
  assert!(
    receiver
      .on_propose(&rewrap(ChequeSpend {
        body: wrong_body,
        payer_signature: signature
      }))
      .is_err()
  );

  // Garbage signature.
  assert!(
    receiver
      .on_propose(&rewrap(ChequeSpend {
        payer_signature: vec![0; 64],
        ..spend.clone()
      }))
      .is_err()
  );
  assert_eq!(
    query_i64(
      &lab.receiver_path(),
      "SELECT count(*) FROM incoming_cheques"
    ),
    0
  );

  // Valid propose stored, then the same cheque with different content (new counter) is refused.
  receiver.on_propose(&propose).unwrap();
  let (conflict_body, _) = spend_body(0);
  let conflict = ChequeSpendBody {
    counter: 2,
    ..ChequeSpendBody::decode(conflict_body.as_slice()).unwrap()
  };
  let conflict_propose = lab
    .open_payer_with_fresh_db("conflict")
    .propose(&conflict.encode_to_vec(), 2, &payee)
    .unwrap();
  assert!(receiver.on_propose(&conflict_propose).is_err());
  assert_eq!(
    query_i64(
      &lab.receiver_path(),
      "SELECT count(*) FROM incoming_cheques"
    ),
    1
  );
}

impl Lab {
  /// A second payer database with the same key, to produce a conflicting signed propose.
  fn open_payer_with_fresh_db(&self, name: &str) -> PayerEngine {
    PayerEngine::open(
      &self.directory.path().join(format!("{name}.db")),
      &DB_KEY,
      &PAYER_SEED,
      &self.payee_public_key,
    )
    .unwrap()
  }
}

#[test]
fn payer_rejects_forged_and_foreign_acks() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();
  let (body, payee) = spend_body(0);
  let propose = payer.propose(&body, 1, &payee).unwrap();
  let ack = AckReceipt::decode(receiver.on_propose(&propose).unwrap().as_slice()).unwrap();

  let bad_signature = AckReceipt {
    payee_signature: vec![0; 64],
    ..ack.clone()
  };
  assert!(payer.on_ack(&bad_signature.encode_to_vec()).is_err());
  let unknown = AckReceipt {
    tx_id: vec![2; 32],
    ..ack.clone()
  };
  assert!(payer.on_ack(&unknown.encode_to_vec()).is_err());
  assert_eq!(payer_states(&lab), (1, 1, 0));

  // Ack signed by the wrong key for the right content.
  let wrong_key = signing_key_from_seed(&[11; 32]).unwrap();
  let forged = AckReceipt {
    payee_signature: meshpay_core::tx_keys::sign(
      &wrong_key,
      &meshpay_core::tx_hashing::ack_signing_message(&ack.tx_id, &ack.spend_hash),
    ),
    ..ack.clone()
  };
  assert!(payer.on_ack(&forged.encode_to_vec()).is_err());
  assert_eq!(payer_states(&lab), (1, 1, 0));
  assert_eq!(
    ack.spend_hash,
    spend_hash(
      &TxPropose::decode(propose.as_slice())
        .unwrap()
        .spend
        .unwrap()
        .body
    )
  );
}

#[test]
fn dart_facing_api_runs_the_full_exchange() {
  let directory = tempfile::tempdir().unwrap();
  let payer_key = generate_lab_signing_key().unwrap();
  let payee_key = generate_lab_signing_key().unwrap();
  let path = |name: &str| directory.path().join(name).to_string_lossy().into_owned();

  let payer = PayerTxEngine::open(
    path("p.db"),
    DB_KEY.to_vec(),
    payer_key.seed.clone(),
    payee_key.public_key.clone(),
  )
  .unwrap();
  let receiver = ReceiverTxEngine::open(
    path("r.db"),
    DB_KEY.to_vec(),
    payee_key.seed,
    payer_key.public_key,
    OWN_ACCOUNT.to_vec(),
  )
  .unwrap();
  let (body, payee) = spend_body(3);
  let propose = payer.propose(body, 4, payee).unwrap();
  let ack = receiver.on_propose(propose.clone()).unwrap();
  assert_eq!(receiver.on_propose(propose).unwrap(), ack);
  payer.on_ack(ack).unwrap();
  assert!(payer.resume_pending().unwrap().is_empty());
}

#[test]
fn databases_are_wal_and_unreadable_without_the_key() {
  let lab = Lab::new();
  drop(lab.open_payer());
  assert!(
    Connection::open(lab.payer_path())
      .unwrap()
      .query_row("SELECT count(*) FROM sqlite_master", [], |row| row
        .get::<_, i64>(0))
      .is_err()
  );
  let connection = Connection::open(lab.payer_path()).unwrap();
  let hex_key: String = DB_KEY.iter().map(|b| format!("{b:02x}")).collect();
  connection
    .pragma_update(None, "key", format!("x'{hex_key}'"))
    .unwrap();
  let mode: String = connection
    .query_row("PRAGMA journal_mode", [], |row| row.get(0))
    .unwrap();
  assert_eq!(mode, "wal");
}

fn sign_spend(body: &[u8], message: &[u8]) -> Vec<u8> {
  let _ = body;
  meshpay_core::tx_keys::sign(&signing_key_from_seed(&PAYER_SEED).unwrap(), message)
}

fn wrap(body: Vec<u8>, signature: Vec<u8>) -> Vec<u8> {
  TxPropose {
    spend: Some(ChequeSpend {
      body,
      payer_signature: signature,
    }),
  }
  .encode_to_vec()
}

#[test]
fn receiver_rejects_cheque_addressed_to_another_account() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();
  let body = ChequeSpendBody {
    cheque_id: b"cheque-other".to_vec(),
    payee_account_id: b"someone-else".to_vec(),
    amount_units: 5,
    counter: 1,
    ..Default::default()
  };
  let propose = payer
    .propose(&body.encode_to_vec(), 1, b"someone-else")
    .unwrap();
  assert!(receiver.on_propose(&propose).is_err());
  assert_eq!(
    query_i64(
      &lab.receiver_path(),
      "SELECT count(*) FROM incoming_cheques"
    ),
    0
  );
}

#[test]
fn spend_signature_is_domain_separated() {
  let lab = Lab::new();
  let receiver = lab.open_receiver();
  let (body, _) = spend_body(0);
  let tx_id = derive_tx_id(1, &body).unwrap();
  let body_bytes = meshpay_core::tx_hashing::body_with_tx_id(&body, &tx_id).unwrap();
  // Signature over the bare body (no domain prefix) must not verify.
  let bare = sign_spend(&body_bytes, &body_bytes);
  assert!(
    receiver
      .on_propose(&wrap(body_bytes.clone(), bare))
      .is_err()
  );
  let prefixed = sign_spend(&body_bytes, &spend_signing_message(&body_bytes));
  assert!(receiver.on_propose(&wrap(body_bytes, prefixed)).is_ok());
}

#[test]
fn tx_id_hashes_the_carried_bytes_not_a_reencode() {
  let (body, _) = spend_body(4);
  let base = derive_tx_id(5, &body).unwrap();
  // Same fields, tx_id field present anywhere in the bytes: same tx_id.
  let with_id = meshpay_core::tx_hashing::body_with_tx_id(&body, &base).unwrap();
  assert_eq!(derive_tx_id(5, &with_id).unwrap(), base);
  // An unknown field changes the carried bytes, so it changes the tx_id (a re-encode would drop it).
  let mut with_unknown = body.clone();
  with_unknown.extend_from_slice(&[0x50, 0x01]); // field 10, varint 1
  assert_ne!(derive_tx_id(5, &with_unknown).unwrap(), base);
  // The counter is part of the preimage.
  assert_ne!(derive_tx_id(6, &body).unwrap(), base);
  assert!(derive_tx_id(5, &[0x22, 0x05, 0x01]).is_err());

  // Receiver accepts a body whose tx_id field sits last (as the payer writes it).
  let lab = Lab::new();
  let payer = lab.open_payer();
  let propose = payer.propose(&body, 5, OWN_ACCOUNT).unwrap();
  assert!(lab.open_receiver().on_propose(&propose).is_ok());
}

#[test]
fn oversized_propose_is_refused_on_both_sides() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();
  let huge = ChequeSpendBody {
    cheque_id: vec![1; 5000],
    payee_account_id: OWN_ACCOUNT.to_vec(),
    counter: 1,
    ..Default::default()
  };
  assert!(
    payer
      .propose(&huge.encode_to_vec(), 1, OWN_ACCOUNT)
      .is_err()
  );
  assert_eq!(payer_states(&lab).0, 0);
  assert!(receiver.on_propose(&vec![0u8; 4097]).is_err());
}

#[test]
fn locked_cheques_expire_only_after_the_window_and_redeliver_until_then() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let (body, payee) = spend_body(0); // signed_at_unix = 1_700_000_000
  let propose = payer.propose(&body, 1, &payee).unwrap();
  assert_eq!(payer.expire_locked(1_700_000_000 + 99, 100).unwrap(), 0);
  assert_eq!(payer.resume_pending().unwrap(), vec![propose.clone()]);
  assert_eq!(payer.expire_locked(1_700_000_000 + 100, 100).unwrap(), 1);
  assert!(payer.resume_pending().unwrap().is_empty());
  assert_eq!(
    query_i64(
      &lab.payer_path(),
      "SELECT count(*) FROM outgoing_cheques WHERE state = 'EXPIRED'"
    ),
    1
  );
  // Committed cheques are never expired.
  let (body2, payee2) = spend_body(1);
  let propose2 = payer.propose(&body2, 2, &payee2).unwrap();
  let ack = lab.open_receiver().on_propose(&propose2).unwrap();
  payer.on_ack(&ack).unwrap();
  assert_eq!(payer.expire_locked(u64::MAX, 0).unwrap(), 0);
  assert_eq!(payer_states(&lab), (2, 0, 1));
}

/// Child half of the SIGKILL test: proposes forever until the parent kills it.
#[test]
fn child_propose_loop() {
  let Ok(directory) = std::env::var("MESHPAY_CHILD_DIR") else {
    return;
  };
  let start: u64 = std::env::var("MESHPAY_CHILD_START")
    .unwrap()
    .parse()
    .unwrap();
  let public = compressed_public_key(&signing_key_from_seed(&PAYEE_SEED).unwrap());
  let payer = PayerEngine::open(
    &Path::new(&directory).join("payer.db"),
    &DB_KEY,
    &PAYER_SEED,
    &public,
  )
  .unwrap();
  println!("ready");
  for counter in start..start + 5000 {
    let (body, payee) = spend_body(counter - 1);
    payer.propose(&body, counter, &payee).unwrap();
  }
}

/// PV-BLE-03 with real process death: 200 SIGKILLs at seeded random moments mid-propose.
#[test]
fn sigkill_during_propose_never_leaves_a_half_written_or_duplicated_cheque() {
  use std::io::{BufRead, BufReader};
  use std::process::{Command, Stdio};
  use std::time::Duration;

  let lab = Lab::new();
  let mut state = 0x9E37_79B9_7F4A_7C15u64;
  let mut next_random = move || {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
  };

  for run in 0..200 {
    let rows = query_i64_if_table(&lab.payer_path());
    let mut child = Command::new(std::env::current_exe().unwrap())
      .args([
        "child_propose_loop",
        "--exact",
        "--nocapture",
        "--test-threads=1",
      ])
      .env("MESHPAY_CHILD_DIR", lab.directory.path())
      .env("MESHPAY_CHILD_START", (rows + 1).to_string())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    loop {
      match lines.next() {
        Some(Ok(line)) if line.ends_with("ready") => break,
        Some(_) => {}
        None => panic!("run {run}: child exited before ready"),
      }
    }
    std::thread::sleep(Duration::from_micros(next_random() % 20_000));
    child.kill().unwrap(); // SIGKILL
    child.wait().unwrap();

    let payer = lab.open_payer();
    let path = lab.payer_path();
    let count = query_i64(&path, "SELECT count(*) FROM outgoing_cheques");
    assert_eq!(
      count,
      query_i64(&path, "SELECT count(DISTINCT tx_id) FROM outgoing_cheques"),
      "run {run}: duplicates"
    );
    assert_eq!(
      count,
      query_i64(
        &path,
        "SELECT coalesce(max(counter), 0) FROM outgoing_cheques"
      ),
      "run {run}: counter gap"
    );
    assert_eq!(
      count,
      query_i64(
        &path,
        "SELECT count(*) FROM outgoing_cheques WHERE state = 'LOCKED_OUTGOING'"
      )
    );
    assert_eq!(
      query_i64(
        &path,
        "SELECT count(*) FROM outgoing_cheques WHERE length(propose_bytes) = 0 OR length(spend_hash) != 32"
      ),
      0,
      "run {run}: half-written row"
    );
    let pending = payer.resume_pending().unwrap();
    assert_eq!(pending.len() as i64, count);
    for (offset, stored) in pending.iter().enumerate().rev().take(20) {
      let counter = offset as u64 + 1;
      let (body, payee) = spend_body(counter - 1);
      assert_eq!(
        &payer.propose(&body, counter, &payee).unwrap(),
        stored,
        "run {run}: bytes changed"
      );
      let spend = TxPropose::decode(stored.as_slice()).unwrap().spend.unwrap();
      assert_eq!(spend_hash(&spend.body).len(), 32);
    }
  }
  assert!(
    query_i64(&lab.payer_path(), "SELECT count(*) FROM outgoing_cheques") >= 200,
    "children made too little progress to exercise the kill"
  );
}

fn query_i64_if_table(path: &Path) -> i64 {
  if !path.exists() {
    return 0;
  }
  query_i64(path, "SELECT count(*) FROM outgoing_cheques")
}
