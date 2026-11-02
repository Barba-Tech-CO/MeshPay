import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/noise/noise_handshake_status.dart';
import 'package:meshpay/core/tx/tx_lab_state.dart';
import 'package:meshpay/features/lab/presentation/helpers/lab_error_message_helper.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_payer_lab_view_model.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_noise_service.dart';
import '../../../../fakes/fake_payer_tx_engine_service.dart';
import '../../../../fakes/fake_permission_service.dart';
import '../../../../fakes/l2cap_receiver_rig.dart';

void main() {
  late L2capReceiverRig rig;
  late FakeNativeLabService native;
  late FakeNoiseService noise;
  late FakePayerTxEngineService payerTx;
  late L2capPayerLabViewModel viewModel;

  setUp(() {
    rig = L2capReceiverRig();
    native = FakeNativeLabService();
    noise = FakeNoiseService();
    payerTx = FakePayerTxEngineService();
    viewModel = L2capPayerLabViewModel(
      native,
      FakePermissionService(),
      noise,
      payerTx,
    );
    rig.linkTo(native);
  });

  tearDown(() async {
    viewModel.dispose();
    await rig.dispose();
  });

  test('loads session, PSM, and key from the receiver QR payload', () async {
    await rig.startAndAccept();

    expect(viewModel.handleQrPayload(rig.viewModel.qrPayload!), isTrue);
    expect(viewModel.sessionIdHex, rig.viewModel.sessionIdHex);
    expect(viewModel.psm, '25');
    expect(viewModel.keyHex, rig.viewModel.keyHex);

    expect(viewModel.handleQrPayload('not json'), isFalse);
    expect(viewModel.error, contains('QR inválido'));
    expect(
      viewModel.handleQrPayload('{"psm":25,"session":"0123456789abcdef"}'),
      isFalse,
    );
    expect(viewModel.error, contains('chave'));
    expect(
      viewModel.handleQrPayload(
        '{"psm":25,"session":"0123456789abcdef","key":"abcd"}',
      ),
      isFalse,
    );
  });

  test(
    'scans, connects with the Noise XK handshake, and measures encrypted echoes',
    () async {
      await rig.startAndAccept();
      viewModel
        ..handleQrPayload(rig.viewModel.qrPayload!)
        ..updateDeviceId('AA:BB');

      await viewModel.startScan();
      expect(native.scanSessionIdHex, viewModel.sessionIdHex);
      viewModel.handleNativeEvent(
        ScanResultEvent(
          deviceId: 'AA:BB',
          rssi: -30,
          sessionIdHex: viewModel.sessionIdHex,
          elapsedRealtimeNanos: 0,
        ),
      );
      expect(viewModel.scanResults.single.deviceId, 'AA:BB');
      viewModel.selectDevice('AA:BB');
      await viewModel.connect();

      expect(native.openedDeviceId, 'AA:BB');
      expect(native.openedPsm, 25);
      expect(viewModel.error, isNull);
      expect(viewModel.isConnected, isTrue);
      expect(viewModel.handshakeStatus, NoiseHandshakeStatus.finished);
      expect(viewModel.handshakeMessageSizes, <int>[48, 48, 64]);
      expect(viewModel.handshakeDurationsMilliseconds, hasLength(1));
      expect(viewModel.handshakeP50Milliseconds, isNotNull);
      expect(viewModel.handshakeP95Milliseconds, isNotNull);
      expect(rig.viewModel.handshakeStatus, NoiseHandshakeStatus.finished);
      // Handshake order on the wire: msg1 (48) then msg3 (64).
      expect(
        native.sentL2capPayloads.map((payload) => payload.length),
        <int>[4 + 48, 4 + 64],
      );
      expect(noise.initiators.single.isFinished, isTrue);

      await viewModel.runEchoBenchmark();

      expect(viewModel.successCount, 5);
      expect(viewModel.p50Milliseconds, isNotNull);
      expect(viewModel.p95Milliseconds, isNotNull);
      final echoes = native.sentL2capPayloads.sublist(2);
      expect(echoes, hasLength(5));
      // 1-byte frame type + 25 KB plaintext + 16-byte tag + 4-byte header.
      expect(
        echoes.every((data) => data.length == 25 * 1024 + 1 + 16 + 4),
        isTrue,
      );
      expect(echoes.first.sublist(0, 4), <int>[0, 0, 0x64, 0x11]);
      expect(echoes.first.sublist(4).every((byte) => byte == 0x5a), isFalse);
      expect(rig.viewModel.receivedMessageCount, 5);

      await viewModel.close();
      expect(viewModel.isConnected, isFalse);
      expect(native.closeL2capCalls, 1);
    },
  );

  test('prologue sent by the payer matches the QR data', () async {
    await rig.startAndAccept();
    final expectedPrologue =
        'meshpay/1|${rig.viewModel.sessionIdHex}|25|${rig.viewModel.keyHex}';
    viewModel
      ..handleQrPayload(rig.viewModel.qrPayload!)
      ..updateDeviceId('AA:BB');

    await viewModel.connect();

    expect(
      String.fromCharCodes(noise.initiatorPrologues.single),
      expectedPrologue,
    );
    expect(
      noise.initiatorPrologues.single,
      rig.noise.responderPrologues.single,
    );
  });

  test(
    'rejects a key with the wrong length without opening a channel',
    () async {
      viewModel
        ..updateSessionIdHex('0123456789abcdef')
        ..updatePsm('25')
        ..updateKeyHex('abcd')
        ..updateDeviceId('AA:BB');

      await viewModel.connect();

      expect(viewModel.error, contains('chave'));
      expect(native.openedDeviceId, isNull);
    },
  );

  test(
    'a modified QR field fails the handshake with a readable error',
    () async {
      await rig.startAndAccept();
      viewModel
        ..handleQrPayload(rig.viewModel.qrPayload!)
        ..updatePsm('26')
        ..updateDeviceId('AA:BB');

      await viewModel.connect();

      expect(viewModel.isConnected, isFalse);
      expect(viewModel.handshakeStatus, NoiseHandshakeStatus.failed);
      expect(viewModel.error, LabErrorMessageHelper.noiseHandshakeMessage);
      expect(viewModel.connectedChannelId, isNull);
    },
  );

  test(
    'a wrong receiver key fails the handshake with a readable error',
    () async {
      await rig.startAndAccept();
      viewModel
        ..handleQrPayload(rig.viewModel.qrPayload!)
        ..updateKeyHex('11' * 32)
        ..updateDeviceId('AA:BB');

      await viewModel.connect();

      expect(viewModel.isConnected, isFalse);
      expect(viewModel.error, LabErrorMessageHelper.noiseHandshakeMessage);
      expect(rig.viewModel.handshakeStatus, NoiseHandshakeStatus.failed);
    },
  );

  test('reports errors and rejects invalid manual connection values', () async {
    viewModel
      ..updateSessionIdHex('bad')
      ..updatePsm('0')
      ..updateDeviceId('');

    await viewModel.connect();

    expect(viewModel.error, contains('sessão'));
    expect(native.openedDeviceId, isNull);
  });

  test('counts corrupted encrypted echoes as failures', () async {
    await rig.startAndAccept();
    viewModel
      ..handleQrPayload(rig.viewModel.qrPayload!)
      ..updateDeviceId('AA:BB');
    await viewModel.connect();
    expect(viewModel.isConnected, isTrue);
    native.mismatchL2capEcho = true;

    await viewModel.runEchoBenchmark();

    expect(viewModel.successCount, 0);
    expect(viewModel.error, isNotNull);
  });

  test('an echo failure closes the channel and stops the benchmark', () async {
    await rig.startAndAccept();
    viewModel
      ..handleQrPayload(rig.viewModel.qrPayload!)
      ..updateDeviceId('AA:BB');
    await viewModel.connect();
    native.mismatchL2capEcho = true;

    await viewModel.runEchoBenchmark();

    // msg1, msg3 and exactly one echo attempt.
    expect(native.sentL2capPayloads, hasLength(3));
    expect(viewModel.isConnected, isFalse);
    expect(native.closeL2capCalls, 1);
  });

  test('an invalid reply frame closes the channel', () async {
    await rig.startAndAccept();
    viewModel
      ..handleQrPayload(rig.viewModel.qrPayload!)
      ..updateDeviceId('AA:BB');
    await viewModel.connect();
    native.l2capResponder = (_, _) async => Uint8List(4);

    await viewModel.runEchoBenchmark();

    expect(viewModel.error, contains('Quadro inválido'));
    expect(viewModel.isConnected, isFalse);
    expect(native.closeL2capCalls, 1);
    expect(viewModel.successCount, 0);
  });

  Future<void> connectPayer(L2capReceiverRig target) async {
    await target.startAndAccept();
    viewModel
      ..handleQrPayload(target.viewModel.qrPayload!)
      ..updateDeviceId('AA:BB');
    await viewModel.connect();
    expect(viewModel.isConnected, isTrue);
  }

  test('runs propose, ack and commit over the encrypted channel', () async {
    await connectPayer(rig);

    await viewModel.sendTestCheque();

    expect(viewModel.error, isNull);
    expect(payerTx.calls, <String>['open', 'propose', 'onAck']);
    // 'close' comes from the end of the charge, before any engine is open.
    expect(rig.tx.calls, <String>['close', 'open', 'onPropose']);
    expect(payerTx.openedPayeeKey, List<int>.filled(33, 3));
    expect(rig.tx.openedPayerKey, List<int>.filled(33, 2));
    // msg1, msg3, hello and propose.
    expect(native.sentL2capPayloads, hasLength(4));
    expect(viewModel.txState, TxLabState.committed);
    expect(rig.viewModel.txState, TxLabState.committed);
    expect(viewModel.txIdHex, hasLength(8));
    expect(rig.viewModel.txIdHex, viewModel.txIdHex);
    expect(rig.tx.proposes.single, payerTx.proposeByCounter.values.single);
    expect(payerTx.resumePendingEmpty, isTrue);
  });

  test('moves through locked, proposed and committed in order', () async {
    await connectPayer(rig);
    final states = <TxLabState>[];
    viewModel.addListener(() {
      if (states.isEmpty || states.last != viewModel.txState) {
        states.add(viewModel.txState);
      }
    });

    await viewModel.sendTestCheque();

    expect(states, <TxLabState>[
      TxLabState.idle,
      TxLabState.locked,
      TxLabState.proposed,
      TxLabState.committed,
    ]);
  });

  test('resend after a dropped link redelivers identical bytes', () async {
    await connectPayer(rig);
    final inner = native.l2capResponder!;
    var sends = 0;
    native.l2capResponder = (channelId, data) async {
      sends++;
      if (sends == 2) {
        throw StateError('link dropped');
      }
      return inner(channelId, data);
    };

    await viewModel.sendTestCheque();

    expect(viewModel.isConnected, isFalse);
    expect(viewModel.txState, TxLabState.locked);
    expect(viewModel.error, contains('Reenviar pendente'));
    expect(rig.tx.proposes, isEmpty);
    final firstBytes = payerTx.proposeByCounter.values.single;
    final txId = viewModel.txIdHex;

    final secondRig = L2capReceiverRig();
    addTearDown(secondRig.dispose);
    secondRig.linkTo(native);
    await connectPayer(secondRig);
    await viewModel.resendPending();

    expect(viewModel.error, isNull);
    expect(secondRig.tx.proposes.single, firstBytes);
    expect(viewModel.txState, TxLabState.committed);
    expect(viewModel.txIdHex, txId);
    expect(secondRig.viewModel.txState, TxLabState.committed);
  });

  test('resend with nothing pending says so', () async {
    await connectPayer(rig);

    await viewModel.resendPending();

    expect(viewModel.error, 'Nenhum cheque pendente para reenviar.');
    expect(viewModel.txState, TxLabState.idle);
  });

  test('shows a receiver refusal in Portuguese on both sides', () async {
    await connectPayer(rig);
    rig.tx.onProposeError = Exception(
      'cheque is not addressed to this account',
    );

    await viewModel.sendTestCheque();

    expect(viewModel.txState, TxLabState.failed);
    expect(
      viewModel.error,
      'Receptor recusou: O cheque não é para esta conta.',
    );
    expect(rig.viewModel.txState, TxLabState.failed);
    expect(rig.viewModel.error, 'O cheque não é para esta conta.');
    expect(viewModel.isConnected, isTrue);
  });

  test('a rejected ack fails the cheque with a readable error', () async {
    await connectPayer(rig);
    payerTx.ackError = Exception('signature verification failed');

    await viewModel.sendTestCheque();

    expect(viewModel.txState, TxLabState.failed);
    expect(viewModel.error, 'A assinatura do cheque ou do recibo não confere.');
  });

  test(
    'error frames carry a fixed code, never the raw exception text',
    () async {
      await connectPayer(rig);
      rig.tx.onProposeError = Exception('secret /data/db path boom');

      await viewModel.sendTestCheque();

      expect(viewModel.error, 'Receptor recusou: Falha interna na transação.');
      expect(rig.viewModel.error, 'Falha interna na transação.');
      expect(viewModel.error, isNot(contains('secret')));
    },
  );

  test('two fast sends use distinct, increasing counters', () async {
    await connectPayer(rig);

    await Future.wait(<Future<void>>[
      viewModel.sendTestCheque(),
      viewModel.sendTestCheque(),
    ]);
    await viewModel.sendTestCheque();

    expect(payerTx.proposeByCounter.keys, <int>[1, 2]);
    expect(rig.tx.proposes, hasLength(2));
  });

  test(
    'resend stops at the first failure and keeps the failed state',
    () async {
      await connectPayer(rig);
      payerTx.ackError = Exception('signature verification failed');
      await viewModel.sendTestCheque();
      await viewModel.sendTestCheque();
      payerTx.ackError = null;
      rig.tx.onProposeError = Exception(
        'cheque is not addressed to this account',
      );
      int proposeCalls() => rig.tx.calls.where((c) => c == 'onPropose').length;
      expect(proposeCalls(), 2);

      await viewModel.resendPending();

      expect(proposeCalls(), 3);
      expect(viewModel.txState, TxLabState.failed);
      expect(
        viewModel.error,
        'Receptor recusou: O cheque não é para esta conta.',
      );
    },
  );

  test('rejects a receiver key of the wrong length', () async {
    await connectPayer(rig);
    rig.tx.publicKey = Uint8List(10);

    await viewModel.sendTestCheque();

    expect(viewModel.txState, TxLabState.failed);
    expect(viewModel.error, 'Chave pública inválida na troca de chaves.');
    expect(payerTx.calls, isNot(contains('propose')));
  });

  test(
    'closing the connection cancels a running send and closes the engine',
    () async {
      await connectPayer(rig);

      final send = viewModel.sendTestCheque();
      await viewModel.close();
      await send;

      expect(viewModel.isConnected, isFalse);
      expect(payerTx.calls, contains('close'));
      expect(viewModel.txState, isNot(TxLabState.committed));
    },
  );
}
