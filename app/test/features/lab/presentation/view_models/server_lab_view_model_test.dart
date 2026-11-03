import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/noise/hex_helper.dart';
import 'package:meshpay/core/server/client/server_error_code.dart';
import 'package:meshpay/core/server/client/server_exception.dart';
import 'package:meshpay/core/server/messages/cheque_stub.dart';
import 'package:meshpay/core/server/messages/server_attestation_level.dart';
import 'package:meshpay/core/server/requests/issue_request_body.dart';
import 'package:meshpay/core/server/responses/issue_response.dart';
import 'package:meshpay/core/server/signing/server_account_id_helper.dart';
import 'package:meshpay/core/server/signing/server_signing_message_helper.dart';
import 'package:meshpay/features/lab/presentation/view_models/server_lab_view_model.dart';

import '../../../../fakes/fake_hardware_key_lab_service.dart';
import '../../../../fakes/fake_payment_key_signer.dart';
import '../../../../fakes/fake_server_client.dart';

// DER (0x30 0x06 0x02 0x01 0x07 0x02 0x01 0x09) -> raw r=7, s=9.
final Uint8List _der = Uint8List.fromList(<int>[48, 6, 2, 1, 7, 2, 1, 9]);
final Uint8List _raw = Uint8List.fromList(<int>[
  ...List<int>.filled(31, 0),
  7,
  ...List<int>.filled(31, 0),
  9,
]);

const int _nowMillis = 1700000000000;

Map<String, String> _golden() => <String, String>{
  for (final line in File(
    '../server/tests/wire/golden.txt',
  ).readAsLinesSync().where((line) => line.isNotEmpty))
    line.substring(0, line.indexOf('=')): line.substring(line.indexOf('=') + 1),
};

void main() {
  late FakeServerClient client;
  late FakeHardwareKeyLabService keys;
  late FakePaymentKeySigner signer;
  late ServerLabViewModel viewModel;

  setUp(() {
    client = FakeServerClient();
    keys = FakeHardwareKeyLabService();
    signer = FakePaymentKeySigner()..signature = _der;
    viewModel = ServerLabViewModel(
      client,
      keys,
      signer,
      clock: () => DateTime.fromMillisecondsSinceEpoch(_nowMillis),
    );
  });

  test(
    'attested register sends chain, server nonce and raw signature',
    () async {
      await viewModel.register();

      final request = client.registers.single;
      expect(
        client.calls.first,
        'challenge:${ServerLabViewModel.defaultBaseUrl}',
      );
      expect(keys.calls, <String>[
        'delete:server_lab_payer',
        'generate:server_lab_payer',
      ]);
      expect(request.attestationChain, hasLength(3));
      expect(request.challenge, client.nonce);
      expect(request.attestationLevel, ServerAttestationLevel.androidTee);
      expect(request.signature, _raw);
      expect(request.publicKey, <int>[3, ...List<int>.filled(32, 171)]);
      expect(
        signer.signedMessages.single,
        ServerSigningMessageHelper.register(request.publicKey),
      );
      expect(viewModel.payer, isNotNull);
      expect(viewModel.error, isNull);
    },
  );

  test('lab register sends no chain, no challenge and LAB_SOFTWARE', () async {
    viewModel.setSendAttestation(false);

    await viewModel.register();

    final request = client.registers.single;
    expect(client.calls, <String>['register']);
    expect(request.attestationChain, isEmpty);
    expect(request.challenge, isEmpty);
    expect(request.attestationLevel, ServerAttestationLevel.labSoftware);
  });

  test('a refused registration shows the typed code, not peer text', () async {
    client.registerError = const ServerException(
      ServerErrorCode.forbidden,
      httpStatus: 403,
    );

    await viewModel.register();

    expect(viewModel.payer, isNull);
    expect(viewModel.error, contains('[forbidden, HTTP 403]'));
  });

  test('issue and deposit need the previous steps', () async {
    await viewModel.issueChequebook();
    expect(viewModel.error, contains('Registre'));

    await viewModel.deposit();
    expect(viewModel.error, contains('talão'));
    expect(client.calls, isEmpty);
  });

  test(
    'full lab flow: issue, deposit to a lab payee, signed balances',
    () async {
      final golden = _golden();
      final golden1 = IssueResponse.fromBytes(
        HexHelper.decode(golden['issue_response']!),
      );
      client.issueResponse = IssueResponse(
        chequebookId: golden1.chequebookId,
        stubs: <ChequeStub>[
          golden1.stubs.first,
          golden1.stubs.first,
          golden1.stubs.first,
        ],
        reserveUnits: golden1.reserveUnits,
      );
      viewModel.setSendAttestation(false);
      await viewModel.register();

      await viewModel.issueChequebook();
      final issue = client.issues.single;
      expect(issue.signature, _raw);
      expect(
        signer.signedMessages.last,
        ServerSigningMessageHelper.issue(issue.body),
      );
      expect(
        issue.body.sublist(0, 18),
        IssueRequestBody(
          accountId: viewModel.payer!.accountId,
          requestId: Uint8List(0),
          chequeCount: 0,
          maxAmountUnits: 0,
          ttlSeconds: 0,
        ).toBytes(),
      );

      await viewModel.deposit();
      final deposit = client.deposits.single;
      expect(
        client.registers.last.attestationLevel,
        ServerAttestationLevel.labSoftware,
      );
      expect(
        deposit.stub.toBytes(),
        client.issueResponse!.stubs.first.toBytes(),
      );
      expect(deposit.spend.payerSignature, _raw);
      expect(
        signer.signedMessages.last,
        ServerSigningMessageHelper.spend(deposit.spend.body),
      );
      expect(signer.calls.last, 'sign:server_lab_payer');
      expect(viewModel.usedStubs, 1);
      expect(viewModel.lastDeposit!.creditedUnits, 50);

      await viewModel.readBalance();
      expect(client.calls.sublist(client.calls.length - 2), <String>[
        'balance:${viewModel.payer!.accountId.first}',
        'balance:${viewModel.payee!.accountId.first}',
      ]);
      expect(
        viewModel.payerBalance!.availableUnits,
        viewModel.payer!.accountId.first,
      );
      expect(
        viewModel.payeeBalance!.availableUnits,
        viewModel.payee!.accountId.first,
      );
      expect(viewModel.payer!.accountId, isNot(viewModel.payee!.accountId));
      expect(
        signer.signedMessages.last,
        ServerSigningMessageHelper.balance(
          viewModel.payee!.accountId,
          _nowMillis ~/ 1000,
        ),
      );
      expect(viewModel.error, isNull);
    },
  );

  test('rejects a register response whose account id is not local', () async {
    client.accountIdOverride = Uint8List(16);

    await viewModel.register();

    expect(viewModel.payer, isNull);
    expect(viewModel.error, contains('[malformedResponse]'));
  });

  test('accepts the locally derived account id', () async {
    await viewModel.register();

    expect(
      viewModel.payer!.accountId,
      ServerAccountIdHelper.derive(client.registers.single.publicKey),
    );
  });

  test('refuses an insecure base url before any request', () async {
    for (final url in <String>[
      'http://example.com',
      'ftp://127.0.0.1',
      'nonsense',
    ]) {
      viewModel.setBaseUrl(url);
      await viewModel.register();
      expect(viewModel.error, contains('[insecureUrl]'));
    }
    viewModel.setBaseUrl('https://example.com');
    await viewModel.register();
    expect(viewModel.error, isNull);
    expect(client.registers, hasLength(1));
    expect(
      keys.calls.where((call) => call.startsWith('generate')),
      hasLength(1),
    );
  });

  test('changing the base url after register resets the state', () async {
    client.issueResponse = IssueResponse(
      chequebookId: Uint8List(1),
      stubs: List<ChequeStub>.generate(
        3,
        (_) => ChequeStub(body: Uint8List(0), serverSignature: Uint8List(0)),
      ),
      reserveUnits: 1,
    );
    await viewModel.register();
    await viewModel.issueChequebook();
    await viewModel.readBalance();

    viewModel.setBaseUrl('http://localhost:1');

    expect(viewModel.payer, isNull);
    expect(viewModel.payee, isNull);
    expect(viewModel.chequebook, isNull);
    expect(viewModel.payerBalance, isNull);
    expect(viewModel.usedStubs, 0);
  });

  test('register resets payee, last deposit and balances', () async {
    client.issueResponse = IssueResponse(
      chequebookId: Uint8List(1),
      stubs: List<ChequeStub>.generate(
        3,
        (_) => ChequeStub(body: Uint8List(0), serverSignature: Uint8List(0)),
      ),
      reserveUnits: 1,
    );
    await viewModel.register();
    await viewModel.issueChequebook();
    await viewModel.deposit();
    await viewModel.readBalance();
    expect(viewModel.payee, isNotNull);

    await viewModel.register();

    expect(viewModel.payee, isNull);
    expect(viewModel.lastDeposit, isNull);
    expect(viewModel.payerBalance, isNull);
    expect(viewModel.payeeBalance, isNull);
  });

  test('rejects an issue response with the wrong stub count', () async {
    client.issueResponse = IssueResponse(
      chequebookId: Uint8List(1),
      stubs: const <ChequeStub>[],
      reserveUnits: 1,
    );
    await viewModel.register();

    await viewModel.issueChequebook();

    expect(viewModel.chequebook, isNull);
    expect(viewModel.error, contains('[malformedResponse]'));
  });
}
