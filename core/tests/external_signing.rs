mod support;

use meshpay_core::api::{PayerTxEngine, ReceiverTxEngine};
use meshpay_core::payer_tx_engine::PayerEngine;
use meshpay_core::receiver_tx_engine::ReceiverEngine;
use meshpay_core::tx_hashing::{
  ack_signing_message, body_with_tx_id, derive_tx_id, spend_signing_message,
};
use meshpay_core::tx_keys::{verify, verifying_key_from_sec1};
use meshpay_core::tx_wire::ack_receipt::AckReceipt;
use meshpay_core::tx_wire::cheque_spend::ChequeSpend;
use meshpay_core::tx_wire::cheque_spend_body::ChequeSpendBody;
use meshpay_core::tx_wire::tx_propose::TxPropose;
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use prost::Message;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const DB_KEY: [u8; 32] = [3; 32];
const OWN_ACCOUNT: &[u8] = b"payee-account";

/// Stands in for the Android Keystore: key never leaves, output is ASN.1 DER like `SHA256withECDSA`.
struct Keystore(SigningKey);

impl Keystore {
  fn new(seed: u8) -> Self {
    Self(SigningKey::from_slice(&[seed; 32]).unwrap())
  }

  fn public_key(&self) -> Vec<u8> {
    self
      .0
      .verifying_key()
      .to_sec1_point(false)
      .as_bytes()
      .to_vec()
  }

  fn sign_raw(&self, message: &[u8]) -> Vec<u8> {
    let signature: Signature = self.0.sign(message);
    signature.to_bytes().to_vec()
  }

  /// DER of the high-S twin (r, n - s) of the signature over `message`.
  fn sign_der_high_s(&self, message: &[u8]) -> Vec<u8> {
    let signature: Signature = self.0.sign(message);
    let (r, s) = signature.split_scalars();
    let twin = Signature::from_scalars(r.to_bytes(), (-*s).to_bytes()).unwrap();
    let high = if twin.normalize_s() != twin {
      twin
    } else {
      signature
    };
    assert_ne!(high.normalize_s(), high, "test signature is high-S");
    high.to_der().as_bytes().to_vec()
  }

  fn sign_der(&self, message: &[u8]) -> Vec<u8> {
    let signature: Signature = self.0.sign(message);
    signature.to_der().as_bytes().to_vec()
  }
}

struct Lab {
  directory: tempfile::TempDir,
  payer: Keystore,
  payee: Keystore,
}

impl Lab {
  fn new() -> Self {
    Self {
      directory: tempfile::tempdir().expect("tempdir"),
      payer: Keystore::new(7),
      payee: Keystore::new(9),
    }
  }

  fn payer_path(&self) -> PathBuf {
    self.directory.path().join("payer.db")
  }

  fn receiver_path(&self) -> PathBuf {
    self.directory.path().join("receiver.db")
  }

  /// Reopening the file is the "crash": state is whatever reached disk.
  fn open_payer(&self) -> PayerEngine {
    PayerEngine::open_external(
      &self.payer_path(),
      &DB_KEY,
      &self.payer.public_key(),
      &self.payee.public_key(),
    )
    .expect("open payer")
  }

  fn open_receiver(&self) -> ReceiverEngine {
    ReceiverEngine::open_external(
      &self.receiver_path(),
      &DB_KEY,
      &self.payee.public_key(),
      &self.payer.public_key(),
      OWN_ACCOUNT,
      support::issuer_trust(),
    )
    .expect("open receiver")
  }
}

fn spend_body(index: u64) -> Vec<u8> {
  ChequeSpendBody {
    cheque_id: format!("cheque-{index:08}").into_bytes(),
    payee_account_id: OWN_ACCOUNT.to_vec(),
    amount_units: 100 + index,
    counter: index + 1,
    signed_at_unix: 1_700_000_000 + index,
    ..Default::default()
  }
  .encode_to_vec()
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

fn payer_rows(lab: &Lab) -> i64 {
  query_i64(&lab.payer_path(), "SELECT count(*) FROM outgoing_cheques")
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

fn propose_externally(lab: &Lab, payer: &PayerEngine, index: u64) -> Vec<u8> {
  let prepared = payer
    .prepare_propose(
      &spend_body(index),
      index + 1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(index)),
    )
    .expect("prepare");
  payer
    .finish_propose(&prepared, &lab.payer.sign_der(&prepared.message))
    .expect("finish")
}

fn ack_externally(lab: &Lab, receiver: &ReceiverEngine, propose: &[u8]) -> Vec<u8> {
  let tx_id = receiver
    .store_propose(propose, &support::context())
    .expect("store");
  let prepared = receiver.prepare_ack(&tx_id).expect("prepare ack");
  receiver
    .finish_ack(&tx_id, &lab.payee.sign_der(&prepared.message))
    .expect("finish ack")
}

#[test]
fn full_two_phase_flow_with_der_signatures() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();

  let propose = propose_externally(&lab, &payer, 0);
  let spend = TxPropose::decode(propose.as_slice())
    .unwrap()
    .spend
    .unwrap();
  assert_eq!(
    spend.payer_signature.len(),
    64,
    "stored signature is normalized raw"
  );
  assert_eq!(payer_states(&lab), (1, 1, 0));

  let ack = ack_externally(&lab, &receiver, &propose);
  assert_eq!(
    AckReceipt::decode(ack.as_slice())
      .unwrap()
      .payee_signature
      .len(),
    64
  );
  payer.on_ack(&ack).expect("payer accepts external ack");
  assert_eq!(payer_states(&lab), (1, 0, 1));
}

#[test]
fn message_to_sign_is_domain_separated_body_with_tx_id() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let prepared = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  assert!(prepared.message.starts_with(b"meshpay/spend/v1"));
  assert_eq!(&prepared.message[16..], prepared.body.as_slice());
  let body = ChequeSpendBody::decode(prepared.body.as_slice()).unwrap();
  assert_eq!(body.tx_id.len(), 32);

  let receiver = lab.open_receiver();
  let propose = propose_externally(&lab, &payer, 0);
  let tx_id = receiver
    .store_propose(&propose, &support::context())
    .unwrap();
  let ack = receiver.prepare_ack(&tx_id).unwrap();
  assert!(ack.message.starts_with(b"meshpay/ack/v1"));
  assert_eq!(&ack.message[14..46], tx_id.as_slice());
  assert!(ack.existing_ack.is_none());
}

#[test]
fn payer_rejects_wrong_key_tampered_message_and_garbage_signatures() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let prepared = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();

  let intruder = Keystore::new(5);
  assert!(
    payer
      .finish_propose(&prepared, &intruder.sign_der(&prepared.message))
      .is_err()
  );
  // Signature over a different message.
  assert!(
    payer
      .finish_propose(&prepared, &lab.payer.sign_der(b"other"))
      .is_err()
  );
  // Tampered body in the handle: the signature no longer matches the rebuilt message.
  let mut tampered = prepared.clone();
  let mut body = ChequeSpendBody::decode(tampered.body.as_slice()).unwrap();
  body.amount_units += 1;
  tampered.body = body.encode_to_vec();
  assert!(
    payer
      .finish_propose(&tampered, &lab.payer.sign_der(&prepared.message))
      .is_err()
  );
  // Message field in the handle is ignored: signing a forged message does not help.
  let mut forged = prepared.clone();
  forged.message = b"forged".to_vec();
  assert!(
    payer
      .finish_propose(&forged, &lab.payer.sign_der(b"forged"))
      .is_err()
  );
  // Truncated DER and empty.
  let mut der = lab.payer.sign_der(&prepared.message);
  der.truncate(der.len() - 3);
  assert!(payer.finish_propose(&prepared, &der).is_err());
  assert!(payer.finish_propose(&prepared, &[]).is_err());
  assert_eq!(payer_rows(&lab), 0, "no rejected attempt wrote a row");

  payer
    .finish_propose(&prepared, &lab.payer.sign_der(&prepared.message))
    .expect("the right signature still works afterwards");
}

#[test]
fn receiver_rejects_wrong_key_and_tampered_ack_signatures() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();
  let propose = propose_externally(&lab, &payer, 0);
  let tx_id = receiver
    .store_propose(&propose, &support::context())
    .unwrap();
  let prepared = receiver.prepare_ack(&tx_id).unwrap();

  assert!(
    receiver
      .finish_ack(&tx_id, &Keystore::new(5).sign_der(&prepared.message))
      .is_err()
  );
  assert!(
    receiver
      .finish_ack(&tx_id, &lab.payee.sign_der(b"other"))
      .is_err()
  );
  assert!(receiver.finish_ack(&tx_id, &[1, 2, 3]).is_err());
  assert!(
    receiver
      .finish_ack(b"unknown", &lab.payee.sign_der(&prepared.message))
      .is_err()
  );
  assert_eq!(
    query_i64(
      &lab.receiver_path(),
      "SELECT count(*) FROM incoming_cheques WHERE state = 'ACKED'"
    ),
    0
  );
  receiver
    .finish_ack(&tx_id, &lab.payee.sign_der(&prepared.message))
    .expect("right signature");
}

#[test]
fn crash_between_prepare_and_finish_leaves_nothing() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let prepared = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  drop(payer);
  assert_eq!(payer_rows(&lab), 0);

  let payer = lab.open_payer();
  assert!(payer.resume_pending().unwrap().is_empty());
  // The old handle is still usable after the restart (it carries everything), exactly once.
  let first = payer
    .finish_propose(&prepared, &lab.payer.sign_der(&prepared.message))
    .unwrap();
  assert_eq!(payer_rows(&lab), 1);
  assert_eq!(payer.resume_pending().unwrap(), vec![first]);

  let receiver = lab.open_receiver();
  let propose = propose_externally(&lab, &payer, 0);
  let tx_id = receiver
    .store_propose(&propose, &support::context())
    .unwrap();
  receiver.prepare_ack(&tx_id).unwrap();
  drop(receiver);
  assert_eq!(
    query_i64(
      &lab.receiver_path(),
      "SELECT count(*) FROM incoming_cheques WHERE state = 'ACKED'"
    ),
    0,
    "prepare_ack wrote nothing"
  );
}

#[test]
fn redelivery_is_byte_identical_even_with_a_different_signature() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let receiver = lab.open_receiver();

  let propose = propose_externally(&lab, &payer, 0);
  let again = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  assert_eq!(again.existing_propose.as_deref(), Some(propose.as_slice()));
  // A fresh (randomized, in real Keystore) signature must not replace the stored one.
  let other = payer
    .finish_propose(&again, &lab.payer.sign_der(&again.message))
    .unwrap();
  assert_eq!(other, propose);
  assert_eq!(payer_rows(&lab), 1);

  let ack = ack_externally(&lab, &receiver, &propose);
  let prepared = receiver
    .prepare_ack(
      &receiver
        .store_propose(&propose, &support::context())
        .unwrap(),
    )
    .unwrap();
  assert_eq!(prepared.existing_ack.as_deref(), Some(ack.as_slice()));
  assert_eq!(
    receiver
      .finish_ack(&prepared.tx_id, &lab.payee.sign_der(&prepared.message))
      .unwrap(),
    ack
  );
  // After a restart the stored ack still comes back unchanged.
  drop(receiver);
  let receiver = lab.open_receiver();
  assert_eq!(ack_externally(&lab, &receiver, &propose), ack);
}

#[test]
fn same_counter_with_different_content_is_refused_in_external_mode() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  propose_externally(&lab, &payer, 0);
  let other = ChequeSpendBody {
    amount_units: 999,
    ..ChequeSpendBody::decode(spend_body(0).as_slice()).unwrap()
  };
  assert!(
    payer
      .prepare_propose(
        &other.encode_to_vec(),
        1,
        OWN_ACCOUNT,
        &support::stub_for(&other.encode_to_vec())
      )
      .is_err()
  );
  assert!(
    payer
      .prepare_propose(
        &spend_body(0),
        2,
        OWN_ACCOUNT,
        &support::stub_for(&spend_body(0))
      )
      .is_err()
  );
}

#[test]
fn external_engines_refuse_the_software_path_and_software_engines_still_work() {
  let lab = Lab::new();
  assert!(
    lab
      .open_payer()
      .propose(
        &spend_body(0),
        1,
        OWN_ACCOUNT,
        &support::stub_for(&spend_body(0))
      )
      .is_err()
  );
  let receiver = lab.open_receiver();
  let propose = propose_externally(&lab, &lab.open_payer(), 0);
  assert!(receiver.on_propose(&propose, &support::context()).is_err());
  assert_eq!(
    query_i64(
      &lab.receiver_path(),
      "SELECT count(*) FROM incoming_cheques"
    ),
    0,
    "on_propose refused before storing anything"
  );

  // Software payer <-> external receiver interoperate: same wire format.
  let software = PayerEngine::open(
    &lab.directory.path().join("software.db"),
    &DB_KEY,
    &[7; 32],
    &lab.payee.public_key(),
  )
  .unwrap();
  let software_propose = software
    .propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  let ack = ack_externally(&lab, &lab.open_receiver(), &software_propose);
  software.on_ack(&ack).unwrap();
}

#[test]
fn dart_facing_api_runs_the_external_exchange() {
  let lab = Lab::new();
  let payer = PayerTxEngine::open_external(
    lab.payer_path().to_string_lossy().into_owned(),
    DB_KEY.to_vec(),
    lab.payer.public_key(),
    lab.payee.public_key(),
  )
  .unwrap();
  let receiver = ReceiverTxEngine::open_external(
    lab.receiver_path().to_string_lossy().into_owned(),
    DB_KEY.to_vec(),
    lab.payee.public_key(),
    lab.payer.public_key(),
    OWN_ACCOUNT.to_vec(),
    support::snapshot_public_key(),
    support::E,
    support::issuer_root_public_key(),
    support::issuer_cert(),
  )
  .unwrap();
  receiver
    .install_snapshot(support::signed_snapshot(support::T_SRV), 0, None)
    .unwrap();

  let prepared = payer
    .prepare_propose(
      spend_body(0),
      1,
      OWN_ACCOUNT.to_vec(),
      support::stub_for(&spend_body(0)),
    )
    .unwrap();
  let propose = payer
    .finish_propose(prepared.clone(), lab.payer.sign_der(&prepared.message))
    .unwrap();
  let tx_id = receiver.store_propose(propose, 0, None).unwrap();
  let prepared_ack = receiver.prepare_ack(tx_id.clone()).unwrap();
  let ack = receiver
    .finish_ack(tx_id, lab.payee.sign_der(&prepared_ack.message))
    .unwrap();
  payer.on_ack(ack).unwrap();
  assert_eq!(payer_states(&lab), (1, 0, 1));
}

/// PV-BLE-03 for external mode: 200 executions, "crash" = engines dropped and DB files reopened
/// at each of five failure points. This covers restart semantics only: what was committed is
/// visible and nothing else is, and redelivery rules hold. Drop is a clean close, so it does NOT
/// model a process dying mid-transaction; that is what the SIGKILL tests below cover.
#[test]
fn two_hundred_crash_runs_in_external_mode() {
  for index in 0..200u64 {
    let lab = Lab::new();
    let payer = lab.open_payer();

    // Failure point 0: crash after prepare, before finish. Nothing may exist; retry works.
    let prepared = payer
      .prepare_propose(
        &spend_body(index),
        index + 1,
        OWN_ACCOUNT,
        &support::stub_for(&spend_body(index)),
      )
      .unwrap();
    drop(payer);
    assert_eq!(payer_rows(&lab), 0, "run {index}: row before finish");
    let payer = lab.open_payer();
    assert!(payer.resume_pending().unwrap().is_empty());
    let propose = payer
      .finish_propose(&prepared, &lab.payer.sign_der(&prepared.message))
      .unwrap();
    assert_eq!(payer_states(&lab), (1, 1, 0), "run {index}");

    let (committed_ack, ack_held_by_payer) = match index % 5 {
      // Failure point 1: payer crashes after finish, before the propose reaches the receiver.
      1 => {
        drop(payer);
        assert_eq!(payer_states(&lab), (1, 1, 0), "run {index}: lost the lock");
        let pending = lab.open_payer().resume_pending().unwrap();
        assert_eq!(pending, vec![propose.clone()], "run {index}: bytes changed");
        (
          ack_externally(&lab, &lab.open_receiver(), &pending[0]),
          None,
        )
      }
      // Failure point 2: payer holds the ack, crashes before on_ack.
      2 => {
        let ack = ack_externally(&lab, &lab.open_receiver(), &propose);
        drop(payer);
        assert_eq!(
          payer_states(&lab),
          (1, 1, 0),
          "run {index}: committed without on_ack"
        );
        (ack.clone(), Some(ack))
      }
      // Failure point 3: receiver crashes after store_propose, before any ack.
      3 => {
        let receiver = lab.open_receiver();
        receiver
          .store_propose(&propose, &support::context())
          .unwrap();
        drop(receiver);
        let receiver = lab.open_receiver();
        assert_eq!(
          query_i64(
            &lab.receiver_path(),
            "SELECT count(*) FROM incoming_cheques WHERE state = 'RECEIVED'"
          ),
          1,
          "run {index}: receiver lost the stored cheque"
        );
        (ack_externally(&lab, &receiver, &propose), None)
      }
      // Failure point 4: receiver crashes after prepare_ack, before finish_ack.
      4 => {
        let receiver = lab.open_receiver();
        let tx_id = receiver
          .store_propose(&propose, &support::context())
          .unwrap();
        let prepared_ack = receiver.prepare_ack(&tx_id).unwrap();
        drop(receiver);
        let receiver = lab.open_receiver();
        assert_eq!(
          query_i64(
            &lab.receiver_path(),
            "SELECT count(*) FROM incoming_cheques WHERE state = 'ACKED'"
          ),
          0,
          "run {index}: ack before finish_ack"
        );
        let ack = receiver
          .finish_ack(&tx_id, &lab.payee.sign_der(&prepared_ack.message))
          .unwrap();
        (ack, None)
      }
      // Failure point 0 again, then the happy path.
      _ => (ack_externally(&lab, &lab.open_receiver(), &propose), None),
    };

    let payer = lab.open_payer();
    payer
      .on_ack(ack_held_by_payer.as_deref().unwrap_or(&committed_ack))
      .unwrap();
    assert_eq!(payer_states(&lab), (1, 0, 1), "run {index}: not committed");
    payer.on_ack(&committed_ack).expect("on_ack idempotent");

    // Late duplicate delivery: byte-identical ack, no second row.
    let receiver = lab.open_receiver();
    assert_eq!(
      ack_externally(&lab, &receiver, &propose),
      committed_ack,
      "run {index}: ack not byte-identical"
    );
    assert_eq!(
      query_i64(
        &lab.receiver_path(),
        "SELECT count(*) FROM incoming_cheques WHERE state = 'ACKED'"
      ),
      1
    );
    assert_eq!(
      query_i64(
        &lab.receiver_path(),
        "SELECT count(*) FROM incoming_cheques"
      ),
      1,
      "run {index}: duplicated incoming cheque"
    );
    let spend: ChequeSpend = TxPropose::decode(propose.as_slice())
      .unwrap()
      .spend
      .unwrap();
    assert_eq!(spend.payer_signature.len(), 64);
  }
}

fn error_of<T>(result: Result<T, String>) -> String {
  match result {
    Ok(_) => panic!("expected an error"),
    Err(error) => error,
  }
}

#[test]
fn high_s_der_is_accepted_and_stored_as_low_s() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let prepared = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  let propose = payer
    .finish_propose(&prepared, &lab.payer.sign_der_high_s(&prepared.message))
    .unwrap();
  let spend = TxPropose::decode(propose.as_slice())
    .unwrap()
    .spend
    .unwrap();
  let stored = Signature::from_slice(&spend.payer_signature).unwrap();
  assert_eq!(stored.normalize_s(), stored, "stored signature is low-S");
  let key = verifying_key_from_sec1(&lab.payer.public_key()).unwrap();
  verify(
    &key,
    &spend_signing_message(&spend.body),
    &spend.payer_signature,
  )
  .unwrap();

  let receiver = lab.open_receiver();
  let tx_id = receiver
    .store_propose(&propose, &support::context())
    .unwrap();
  let ack_prepared = receiver.prepare_ack(&tx_id).unwrap();
  receiver
    .finish_ack(&tx_id, &lab.payee.sign_der_high_s(&ack_prepared.message))
    .expect("high-S ack signature");
}

#[test]
fn malformed_signature_encodings_are_rejected() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let prepared = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  let der = lab.payer.sign_der(&prepared.message);
  let raw = lab.payer.sign_raw(&prepared.message);

  let mut trailing = der.clone();
  trailing.push(0);
  let zero_r = [vec![0u8; 32], raw[32..].to_vec()].concat();
  let zero_s = [raw[..32].to_vec(), vec![0u8; 32]].concat();
  // DER with r = 0, then with a negative (high bit set, no padding byte) r.
  let der_zero_r = [vec![
    0x30, 0x08, 0x02, 0x01, 0x00, 0x02, 0x03, 0x01, 0x02, 0x03,
  ]]
  .concat();
  let der_negative_r = vec![0x30, 0x08, 0x02, 0x01, 0x80, 0x02, 0x03, 0x01, 0x02, 0x03];
  // 64 bytes that look like DER (sequence header + padding) but are neither a valid raw nor DER.
  let der_looking_64 = [vec![0x30, 0x3e], vec![0x02; 62]].concat();
  assert_eq!(der_looking_64.len(), 64);

  for (name, bad) in [
    ("trailing bytes", trailing),
    ("zero r (raw)", zero_r),
    ("zero s (raw)", zero_s),
    ("zero r (DER)", der_zero_r),
    ("negative r (DER)", der_negative_r),
    ("64-byte DER-looking", der_looking_64),
    ("raw + 1 byte", [raw.clone(), vec![0]].concat()),
    ("63 bytes", raw[..63].to_vec()),
    ("all 0xff", vec![0xff; 64]),
  ] {
    assert!(
      payer.finish_propose(&prepared, &bad).is_err(),
      "{name} accepted"
    );
  }
  assert_eq!(payer_rows(&lab), 0);
  payer
    .finish_propose(&prepared, &raw)
    .expect("raw 64 bytes also accepted");
}

#[test]
fn compressed_key_is_accepted_and_identity_or_off_curve_keys_are_refused() {
  let lab = Lab::new();
  let compressed = lab
    .payer
    .0
    .verifying_key()
    .to_sec1_point(true)
    .as_bytes()
    .to_vec();
  assert_eq!(compressed.len(), 33);
  let payer = PayerEngine::open_external(
    &lab.payer_path(),
    &DB_KEY,
    &compressed,
    &lab.payee.public_key(),
  )
  .expect("compressed key");
  let prepared = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  payer
    .finish_propose(&prepared, &lab.payer.sign_der(&prepared.message))
    .expect("signature verifies against the compressed key");

  let off_curve = [vec![4u8], vec![1u8; 64]].concat();
  for (name, key) in [
    ("identity", vec![0u8]),
    ("empty", vec![]),
    ("off curve", off_curve),
    ("short", vec![2u8; 10]),
  ] {
    let path = lab.directory.path().join(format!("bad-{name}.db"));
    assert!(
      PayerEngine::open_external(&path, &DB_KEY, &key, &lab.payee.public_key()).is_err(),
      "payer accepted {name} own key"
    );
    assert!(
      ReceiverEngine::open_external(
        &path,
        &DB_KEY,
        &key,
        &lab.payer.public_key(),
        OWN_ACCOUNT,
        support::issuer_trust()
      )
      .is_err(),
      "receiver accepted {name} own key"
    );
  }
}

#[test]
fn handle_with_another_body_and_its_correct_tx_id_fails_on_the_signature() {
  let lab = Lab::new();
  let payer = lab.open_payer();
  let signed = payer
    .prepare_propose(
      &spend_body(0),
      1,
      OWN_ACCOUNT,
      &support::stub_for(&spend_body(0)),
    )
    .unwrap();
  let other_body = ChequeSpendBody {
    amount_units: 777,
    ..ChequeSpendBody::decode(spend_body(0).as_slice()).unwrap()
  }
  .encode_to_vec();
  // Attacker handle: a different body that carries its own, correctly derived tx_id.
  let other_tx_id = derive_tx_id(1, &other_body).unwrap();
  let handle_body = body_with_tx_id(&other_body, &other_tx_id).unwrap();
  assert_eq!(
    ChequeSpendBody::decode(handle_body.as_slice())
      .unwrap()
      .tx_id,
    other_tx_id
  );
  let mut handle = signed.clone();
  handle.body = handle_body;

  let error = error_of(payer.finish_propose(&handle, &lab.payer.sign_der(&signed.message)));
  assert!(
    error.contains("signature verification failed"),
    "got: {error}"
  );
  assert_eq!(payer_rows(&lab), 0);
}

/// Child half of the finish_propose SIGKILL test: prepares, signs and finishes forever.
#[test]
fn child_finish_propose_loop() {
  let Ok(directory) = std::env::var("MESHPAY_CHILD_DIR") else {
    return;
  };
  let start: u64 = std::env::var("MESHPAY_CHILD_START")
    .unwrap()
    .parse()
    .unwrap();
  let (payer, payee) = (Keystore::new(7), Keystore::new(9));
  let engine = PayerEngine::open_external(
    &Path::new(&directory).join("payer.db"),
    &DB_KEY,
    &payer.public_key(),
    &payee.public_key(),
  )
  .unwrap();
  println!("ready");
  for counter in start..start + 5000 {
    let prepared = engine
      .prepare_propose(
        &spend_body(counter - 1),
        counter,
        OWN_ACCOUNT,
        &support::stub_for(&spend_body(counter - 1)),
      )
      .unwrap();
    engine
      .finish_propose(&prepared, &payer.sign_der(&prepared.message))
      .unwrap();
  }
}

/// A valid signed propose built by hand (what a payer device would send), no engine involved.
fn forged_propose(lab: &Lab, index: u64) -> Vec<u8> {
  let body_bytes = {
    let body = spend_body(index);
    body_with_tx_id(&body, &derive_tx_id(index + 1, &body).unwrap()).unwrap()
  };
  TxPropose {
    stub: Some(support::stub_message_for(&body_bytes)),
    spend: Some(ChequeSpend {
      payer_signature: lab.payer.sign_raw(&spend_signing_message(&body_bytes)),
      body: body_bytes,
    }),
  }
  .encode_to_vec()
}

/// Child half of the finish_ack SIGKILL test: store, prepare and finish acks forever.
#[test]
fn child_finish_ack_loop() {
  let Ok(directory) = std::env::var("MESHPAY_CHILD_DIR") else {
    return;
  };
  let start: u64 = std::env::var("MESHPAY_CHILD_START")
    .unwrap()
    .parse()
    .unwrap();
  let lab = Lab::new();
  let receiver = ReceiverEngine::open_external(
    &Path::new(&directory).join("receiver.db"),
    &DB_KEY,
    &lab.payee.public_key(),
    &lab.payer.public_key(),
    OWN_ACCOUNT,
    support::issuer_trust(),
  )
  .unwrap();
  println!("ready");
  for index in start..start + 5000 {
    ack_externally(&lab, &receiver, &forged_propose(&lab, index));
  }
}

/// Spawns the named child test, waits for "ready", kills it with SIGKILL after a random delay.
fn run_and_kill_child(child_test: &str, directory: &Path, start: u64, delay_micros: u64, run: u32) {
  use std::io::{BufRead, BufReader};
  use std::process::{Command, Stdio};
  use std::time::Duration;

  let mut child = Command::new(std::env::current_exe().unwrap())
    .args([child_test, "--exact", "--nocapture", "--test-threads=1"])
    .env("MESHPAY_CHILD_DIR", directory)
    .env("MESHPAY_CHILD_START", start.to_string())
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
  std::thread::sleep(Duration::from_micros(delay_micros));
  child.kill().unwrap(); // SIGKILL
  child.wait().unwrap();
}

fn xorshift(state: &mut u64) -> u64 {
  *state ^= *state << 13;
  *state ^= *state >> 7;
  *state ^= *state << 17;
  *state
}

const SIGKILL_RUNS: u32 = 120;

#[test]
fn sigkill_during_finish_propose_never_leaves_a_half_written_or_duplicated_cheque() {
  let lab = Lab::new();
  let mut state = 0x9E37_79B9_7F4A_7C15u64;
  let payer_key = verifying_key_from_sec1(&lab.payer.public_key()).unwrap();

  for run in 0..SIGKILL_RUNS {
    let rows = if lab.payer_path().exists() {
      payer_rows(&lab)
    } else {
      0
    };
    run_and_kill_child(
      "child_finish_propose_loop",
      lab.directory.path(),
      rows as u64 + 1,
      xorshift(&mut state) % 20_000,
      run,
    );

    let payer = lab.open_payer();
    let path = lab.payer_path();
    let count = payer_rows(&lab);
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
      let spend = TxPropose::decode(stored.as_slice()).unwrap().spend.unwrap();
      verify(
        &payer_key,
        &spend_signing_message(&spend.body),
        &spend.payer_signature,
      )
      .unwrap_or_else(|error| panic!("run {run}: stored signature invalid: {error}"));
      let prepared = payer
        .prepare_propose(
          &spend_body(counter - 1),
          counter,
          OWN_ACCOUNT,
          &support::stub_for(&spend_body(counter - 1)),
        )
        .unwrap();
      assert_eq!(
        prepared.existing_propose.as_ref(),
        Some(stored),
        "run {run}: bytes changed"
      );
      let again = payer
        .finish_propose(&prepared, &lab.payer.sign_der(&prepared.message))
        .unwrap();
      assert_eq!(&again, stored, "run {run}: redelivery differs");
    }
    assert_eq!(payer_rows(&lab), count, "run {run}: redelivery added rows");
  }
  assert!(
    payer_rows(&lab) >= 100,
    "children made too little progress to exercise the kill"
  );
}

#[test]
fn sigkill_during_finish_ack_never_leaves_a_half_written_or_duplicated_ack() {
  let lab = Lab::new();
  let mut state = 0xD1B5_4A32_D192_ED03u64;
  let payee_key = verifying_key_from_sec1(&lab.payee.public_key()).unwrap();
  let incoming = |sql: &str| query_i64(&lab.receiver_path(), sql);

  for run in 0..SIGKILL_RUNS {
    let rows = if lab.receiver_path().exists() {
      incoming("SELECT count(*) FROM incoming_cheques")
    } else {
      0
    };
    run_and_kill_child(
      "child_finish_ack_loop",
      lab.directory.path(),
      rows as u64,
      xorshift(&mut state) % 20_000,
      run,
    );

    let receiver = lab.open_receiver();
    let count = incoming("SELECT count(*) FROM incoming_cheques");
    assert_eq!(
      count,
      incoming("SELECT count(DISTINCT tx_id) FROM incoming_cheques"),
      "run {run}: duplicates"
    );
    // Sequential child: at most the one in-flight cheque can lack its ack.
    assert!(
      incoming("SELECT count(*) FROM incoming_cheques WHERE state = 'RECEIVED'") <= 1,
      "run {run}"
    );
    assert_eq!(
      incoming(
        "SELECT count(*) FROM incoming_cheques WHERE (state = 'ACKED') != (length(coalesce(ack_bytes, x'')) > 0)"
      ),
      0,
      "run {run}: half-written ack"
    );
    for index in (0..count as u64).rev().take(20) {
      let propose = forged_propose(&lab, index);
      let tx_id = receiver
        .store_propose(&propose, &support::context())
        .unwrap();
      let prepared = receiver.prepare_ack(&tx_id).unwrap();
      let ack = receiver
        .finish_ack(&tx_id, &lab.payee.sign_der(&prepared.message))
        .unwrap();
      if let Some(stored) = &prepared.existing_ack {
        assert_eq!(&ack, stored, "run {run}: stored ack changed");
      }
      let parsed = AckReceipt::decode(ack.as_slice()).unwrap();
      verify(
        &payee_key,
        &ack_signing_message(&parsed.tx_id, &parsed.spend_hash),
        &parsed.payee_signature,
      )
      .unwrap_or_else(|error| panic!("run {run}: ack signature invalid: {error}"));
      assert_eq!(
        receiver.prepare_ack(&tx_id).unwrap().existing_ack.as_ref(),
        Some(&ack),
        "run {run}: ack not stored"
      );
    }
    assert_eq!(
      incoming("SELECT count(*) FROM incoming_cheques"),
      count,
      "run {run}: redelivery added rows"
    );
    assert_eq!(
      incoming("SELECT count(*) FROM incoming_cheques WHERE state = 'RECEIVED'"),
      0,
      "run {run}: unfinished after recovery"
    );
  }
  assert!(
    incoming("SELECT count(*) FROM incoming_cheques") >= 100,
    "children made too little progress to exercise the kill"
  );
}
