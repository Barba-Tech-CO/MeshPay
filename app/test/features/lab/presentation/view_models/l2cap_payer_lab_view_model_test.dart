import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/noise/noise_handshake_status.dart';
import 'package:meshpay/features/lab/presentation/helpers/lab_error_message_helper.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_payer_lab_view_model.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_noise_service.dart';
import '../../../../fakes/fake_permission_service.dart';
import '../../../../fakes/l2cap_receiver_rig.dart';

void main() {
  late L2capReceiverRig rig;
  late FakeNativeLabService native;
  late FakeNoiseService noise;
  late L2capPayerLabViewModel viewModel;

  setUp(() {
    rig = L2capReceiverRig();
    native = FakeNativeLabService();
    noise = FakeNoiseService();
    viewModel = L2capPayerLabViewModel(
      native,
      FakePermissionService(),
      noise,
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
      // 25 KB plaintext + 16-byte tag + 4-byte header.
      expect(echoes.every((data) => data.length == 25 * 1024 + 16 + 4), isTrue);
      expect(echoes.first.sublist(0, 4), <int>[0, 0, 0x64, 0x10]);
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
}
