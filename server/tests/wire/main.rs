//! Golden wire vectors shared with the Flutter client (app/test/core/server/server_wire_golden_test.dart).
//! The Dart test reads golden.txt directly, so both sides pin the same bytes.
use meshpay_server::crypto::signing_messages::{
  balance_message, issue_message, register_message, spend_signing_message,
};
use meshpay_server::crypto::spend_hashing::derive_tx_id;
use meshpay_server::wire::attestation_level::AttestationLevel;
use meshpay_server::wire::balance_response::BalanceResponse;
use meshpay_server::wire::challenge_response::ChallengeResponse;
use meshpay_server::wire::cheque_spend::ChequeSpend;
use meshpay_server::wire::cheque_spend_body::ChequeSpendBody;
use meshpay_server::wire::cheque_stub::ChequeStub;
use meshpay_server::wire::cheque_stub_body::ChequeStubBody;
use meshpay_server::wire::conflict_kind::ConflictKind;
use meshpay_server::wire::deposit_request::DepositRequest;
use meshpay_server::wire::deposit_response::DepositResponse;
use meshpay_server::wire::equivocation_proof::EquivocationProof;
use meshpay_server::wire::issue_request::IssueRequest;
use meshpay_server::wire::issue_request_body::IssueRequestBody;
use meshpay_server::wire::issue_response::IssueResponse;
use meshpay_server::wire::redeem_status::RedeemStatus;
use meshpay_server::wire::register_request::RegisterRequest;
use meshpay_server::wire::register_response::RegisterResponse;
use prost::Message;

fn filled(len: usize, value: u8) -> Vec<u8> {
  vec![value; len]
}

fn stub_body() -> ChequeStubBody {
  ChequeStubBody {
    cheque_id: filled(16, 0x11),
    chequebook_id: filled(16, 0x22),
    payer_account_id: filled(16, 0x33),
    payer_epoch: 1,
    signer_pubkey: filled(65, 0x04),
    max_amount_units: 100,
    expires_at_unix: 1_800_000_000,
    attestation_level: AttestationLevel::LabSoftware as i32,
    single_use_key: false,
    server_key_id: filled(16, 0x44),
  }
}

fn stub() -> ChequeStub {
  ChequeStub {
    body: stub_body().encode_to_vec(),
    server_signature: filled(64, 0x55),
  }
}

fn spend_body() -> ChequeSpendBody {
  ChequeSpendBody {
    cheque_id: filled(16, 0x11),
    payee_account_id: filled(16, 0x66),
    amount_units: 50,
    tx_id: filled(32, 0x77),
    counter: 3,
    prev_spend_hash: Vec::new(),
    signed_at_unix: 1_700_000_000,
  }
}

fn spend() -> ChequeSpend {
  ChequeSpend {
    body: spend_body().encode_to_vec(),
    payer_signature: filled(64, 0x88),
  }
}

fn vectors() -> Vec<(&'static str, Vec<u8>)> {
  let without_tx_id = ChequeSpendBody {
    tx_id: Vec::new(),
    ..spend_body()
  }
  .encode_to_vec();
  vec![
    (
      "register_request",
      RegisterRequest {
        public_key: filled(65, 0x04),
        signature: filled(64, 0x01),
        attestation_level: AttestationLevel::AndroidTee as i32,
        attestation_chain: vec![filled(3, 0xa1), filled(2, 0xa2)],
        challenge: filled(32, 0x0c),
      }
      .encode_to_vec(),
    ),
    (
      "register_request_lab",
      RegisterRequest {
        public_key: filled(65, 0x04),
        signature: filled(64, 0x01),
        attestation_level: AttestationLevel::LabSoftware as i32,
        attestation_chain: Vec::new(),
        challenge: Vec::new(),
      }
      .encode_to_vec(),
    ),
    (
      "challenge_response",
      ChallengeResponse {
        nonce: filled(32, 0x0c),
        expires_at_unix: 1_800_000_060,
      }
      .encode_to_vec(),
    ),
    (
      "register_response",
      RegisterResponse {
        account_id: filled(16, 0x33),
        epoch: 1,
        created: true,
        available_units: 10_000,
      }
      .encode_to_vec(),
    ),
    (
      "issue_request_body",
      IssueRequestBody {
        account_id: filled(16, 0x33),
        request_id: filled(16, 0x99),
        cheque_count: 3,
        max_amount_units: 100,
        ttl_seconds: 3600,
      }
      .encode_to_vec(),
    ),
    (
      "issue_request",
      IssueRequest {
        body: vec![1, 2, 3],
        signature: filled(64, 0x01),
      }
      .encode_to_vec(),
    ),
    ("cheque_stub_body", stub_body().encode_to_vec()),
    (
      "issue_response",
      IssueResponse {
        chequebook_id: filled(16, 0x22),
        stubs: vec![stub(), stub()],
        reserve_units: 300,
      }
      .encode_to_vec(),
    ),
    ("cheque_spend_body", spend_body().encode_to_vec()),
    (
      "deposit_request",
      DepositRequest {
        stub: Some(stub()),
        spend: Some(spend()),
      }
      .encode_to_vec(),
    ),
    (
      "deposit_response_paid",
      DepositResponse {
        status: RedeemStatus::Paid as i32,
        detail: String::new(),
        proof: None,
        credited_units: 50,
      }
      .encode_to_vec(),
    ),
    (
      "deposit_response_conflict",
      DepositResponse {
        status: RedeemStatus::RejectedConflict as i32,
        detail: "conflict".to_owned(),
        proof: Some(EquivocationProof {
          offender_account_id: filled(16, 0x33),
          epoch: 1,
          conflict_kind: ConflictKind::SameCheque as i32,
          stub_1: Some(stub()),
          spend_1: Some(spend()),
          stub_2: None,
          spend_2: None,
        }),
        credited_units: 0,
      }
      .encode_to_vec(),
    ),
    (
      "balance_response",
      BalanceResponse {
        available_units: 9_700,
        reserved_units: 300,
        frozen: false,
      }
      .encode_to_vec(),
    ),
    ("register_message", register_message(&filled(2, 0x04))),
    ("issue_message", issue_message(&[1, 2, 3])),
    ("spend_message", spend_signing_message(&[1, 2, 3])),
    (
      "balance_message",
      balance_message(&filled(16, 0x33), 1_700_000_000),
    ),
    (
      "tx_id",
      derive_tx_id(3, &spend_body().encode_to_vec()).expect("tx_id"),
    ),
    ("spend_body_without_tx_id", without_tx_id),
  ]
}

#[test]
fn wire_golden_vectors_are_stable() {
  let rendered: String = vectors()
    .into_iter()
    .map(|(name, bytes)| format!("{name}={}\n", hex::encode(bytes)))
    .collect();
  let expected = include_str!("golden.txt");
  if rendered != expected {
    println!("{rendered}");
  }
  assert_eq!(rendered, expected);
}
