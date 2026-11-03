import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/noise/hex_helper.dart';
import 'package:meshpay/core/server/messages/cheque_spend.dart';
import 'package:meshpay/core/server/messages/cheque_spend_body.dart';
import 'package:meshpay/core/server/messages/cheque_stub.dart';
import 'package:meshpay/core/server/messages/cheque_stub_body.dart';
import 'package:meshpay/core/server/messages/redeem_status.dart';
import 'package:meshpay/core/server/messages/server_attestation_level.dart';
import 'package:meshpay/core/server/requests/deposit_request.dart';
import 'package:meshpay/core/server/requests/issue_request.dart';
import 'package:meshpay/core/server/requests/issue_request_body.dart';
import 'package:meshpay/core/server/requests/register_request.dart';
import 'package:meshpay/core/server/responses/balance_response.dart';
import 'package:meshpay/core/server/responses/challenge_response.dart';
import 'package:meshpay/core/server/responses/deposit_response.dart';
import 'package:meshpay/core/server/responses/issue_response.dart';
import 'package:meshpay/core/server/responses/register_response.dart';
import 'package:meshpay/core/server/signing/server_signing_message_helper.dart';
import 'package:meshpay/core/server/signing/spend_tx_id_helper.dart';

// Bytes produced by prost in server/tests/wire/main.rs.
final Map<String, String> _golden = <String, String>{
  for (final line in File(
    '../server/tests/wire/golden.txt',
  ).readAsLinesSync().where((line) => line.isNotEmpty))
    line.substring(0, line.indexOf('=')): line.substring(line.indexOf('=') + 1),
};

Uint8List _filled(int length, int value) =>
    Uint8List.fromList(List<int>.filled(length, value));

Uint8List _vector(String name) => HexHelper.decode(_golden[name]!);

ChequeStub _stub() => ChequeStub(
  body: _vector('cheque_stub_body'),
  serverSignature: _filled(64, 0x55),
);

ChequeSpendBody _spendBody({required bool withTxId}) => ChequeSpendBody(
  chequeId: _filled(16, 0x11),
  payeeAccountId: _filled(16, 0x66),
  amountUnits: 50,
  txId: withTxId ? _filled(32, 0x77) : Uint8List(0),
  counter: 3,
  prevSpendHash: Uint8List(0),
  signedAtUnix: 1700000000,
);

void main() {
  test('attested register request matches prost', () {
    final request = RegisterRequest(
      publicKey: _filled(65, 0x04),
      signature: _filled(64, 0x01),
      attestationLevel: ServerAttestationLevel.androidTee,
      attestationChain: <Uint8List>[_filled(3, 0xa1), _filled(2, 0xa2)],
      challenge: _filled(32, 0x0c),
    );

    expect(request.toBytes(), _vector('register_request'));
  });

  test('lab register request matches prost', () {
    final request = RegisterRequest(
      publicKey: _filled(65, 0x04),
      signature: _filled(64, 0x01),
      attestationLevel: ServerAttestationLevel.labSoftware,
      attestationChain: const <Uint8List>[],
      challenge: Uint8List(0),
    );

    expect(request.toBytes(), _vector('register_request_lab'));
  });

  test('issue request and body match prost', () {
    final body = IssueRequestBody(
      accountId: _filled(16, 0x33),
      requestId: _filled(16, 0x99),
      chequeCount: 3,
      maxAmountUnits: 100,
      ttlSeconds: 3600,
    );
    final request = IssueRequest(
      body: Uint8List.fromList(<int>[1, 2, 3]),
      signature: _filled(64, 0x01),
    );

    expect(body.toBytes(), _vector('issue_request_body'));
    expect(request.toBytes(), _vector('issue_request'));
  });

  test('spend body and deposit request match prost', () {
    final spend = ChequeSpend(
      body: _spendBody(withTxId: true).toBytes(),
      payerSignature: _filled(64, 0x88),
    );

    expect(_spendBody(withTxId: true).toBytes(), _vector('cheque_spend_body'));
    expect(
      _spendBody(withTxId: false).toBytes(),
      _vector('spend_body_without_tx_id'),
    );
    expect(
      DepositRequest(stub: _stub(), spend: spend).toBytes(),
      _vector('deposit_request'),
    );
  });

  test('decodes challenge, register and balance responses', () {
    final challenge = ChallengeResponse.fromBytes(
      _vector('challenge_response'),
    );
    final register = RegisterResponse.fromBytes(_vector('register_response'));
    final balance = BalanceResponse.fromBytes(_vector('balance_response'));

    expect(challenge.nonce, _filled(32, 0x0c));
    expect(challenge.expiresAtUnix, 1800000060);
    expect(register.accountId, _filled(16, 0x33));
    expect(register.epoch, 1);
    expect(register.created, isTrue);
    expect(register.availableUnits, 10000);
    expect(balance.availableUnits, 9700);
    expect(balance.reservedUnits, 300);
    expect(balance.frozen, isFalse);
  });

  test('decodes the issue response and its stub bodies', () {
    final response = IssueResponse.fromBytes(_vector('issue_response'));
    final body = ChequeStubBody.fromBytes(response.stubs.first.body);

    expect(response.chequebookId, _filled(16, 0x22));
    expect(response.reserveUnits, 300);
    expect(response.stubs, hasLength(2));
    expect(response.stubs.first.serverSignature, _filled(64, 0x55));
    expect(response.stubs.first.toBytes(), _stub().toBytes());
    expect(body.chequeId, _filled(16, 0x11));
    expect(body.payerAccountId, _filled(16, 0x33));
    expect(body.payerEpoch, 1);
    expect(body.signerPubkey, _filled(65, 0x04));
    expect(body.maxAmountUnits, 100);
    expect(body.expiresAtUnix, 1800000000);
    expect(body.attestationLevel, ServerAttestationLevel.labSoftware);
    expect(body.singleUseKey, isFalse);
    expect(body.serverKeyId, _filled(16, 0x44));
  });

  test('decodes paid and conflict deposit responses', () {
    final paid = DepositResponse.fromBytes(_vector('deposit_response_paid'));
    final conflict = DepositResponse.fromBytes(
      _vector('deposit_response_conflict'),
    );

    expect(paid.status, RedeemStatus.paid);
    expect(paid.creditedUnits, 50);
    expect(paid.hasProof, isFalse);
    expect(conflict.status, RedeemStatus.rejectedConflict);
    expect(conflict.hasProof, isTrue);
    expect(conflict.creditedUnits, 0);
  });

  test('signing messages and tx_id match the server', () {
    expect(
      ServerSigningMessageHelper.register(_filled(2, 0x04)),
      _vector('register_message'),
    );
    expect(
      ServerSigningMessageHelper.issue(Uint8List.fromList(<int>[1, 2, 3])),
      _vector('issue_message'),
    );
    expect(
      ServerSigningMessageHelper.spend(Uint8List.fromList(<int>[1, 2, 3])),
      _vector('spend_message'),
    );
    expect(
      ServerSigningMessageHelper.balance(_filled(16, 0x33), 1700000000),
      _vector('balance_message'),
    );
    expect(
      SpendTxIdHelper.derive(3, _spendBody(withTxId: true).toBytes()),
      isNot(_vector('tx_id')),
    );
    expect(
      SpendTxIdHelper.derive(3, _spendBody(withTxId: false).toBytes()),
      _vector('tx_id'),
    );
  });
}
