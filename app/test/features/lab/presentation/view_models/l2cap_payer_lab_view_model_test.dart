import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_payer_lab_view_model.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_permission_service.dart';

void main() {
  test('loads session and PSM from the receiver QR payload', () {
    final viewModel = L2capPayerLabViewModel(
      FakeNativeLabService(),
      FakePermissionService(),
    );

    expect(
      viewModel.handleQrPayload(
        '{"psm":25,"session":"0123456789abcdef"}',
      ),
      isTrue,
    );
    expect(viewModel.sessionIdHex, '0123456789abcdef');
    expect(viewModel.psm, '25');

    expect(viewModel.handleQrPayload('not json'), isFalse);
    expect(viewModel.error, contains('QR inválido'));
    viewModel.dispose();
  });

  test(
    'scans the session, connects, and measures five framed echoes',
    () async {
      final native = FakeNativeLabService();
      final viewModel =
          L2capPayerLabViewModel(
              native,
              FakePermissionService(),
            )
            ..updateSessionIdHex('0123456789abcdef')
            ..updatePsm('25');

      await viewModel.startScan();
      expect(native.scanSessionIdHex, viewModel.sessionIdHex);
      viewModel.handleNativeEvent(
        const ScanResultEvent(
          deviceId: 'AA:BB',
          rssi: -30,
          sessionIdHex: '0123456789abcdef',
          elapsedRealtimeNanos: 0,
        ),
      );
      expect(viewModel.scanResults.single.deviceId, 'AA:BB');
      viewModel.selectDevice('AA:BB');
      await viewModel.connect();

      expect(native.openedDeviceId, 'AA:BB');
      expect(native.openedPsm, 25);
      expect(viewModel.isConnected, isTrue);

      await viewModel.runEchoBenchmark();

      expect(viewModel.successCount, 5);
      expect(viewModel.p50Milliseconds, isNotNull);
      expect(viewModel.p95Milliseconds, isNotNull);
      expect(native.sentL2capPayloads, hasLength(5));
      expect(
        native.sentL2capPayloads.every((data) => data.length == 25 * 1024 + 4),
        isTrue,
      );
      expect(native.sentL2capPayloads.first.sublist(0, 4), <int>[
        0,
        0,
        0x64,
        0,
      ]);

      await viewModel.close();
      expect(viewModel.isConnected, isFalse);
      expect(native.closeL2capCalls, 1);
      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);
    },
  );

  test('reports errors and rejects invalid manual connection values', () async {
    final native = FakeNativeLabService();
    final viewModel =
        L2capPayerLabViewModel(
            native,
            FakePermissionService(),
          )
          ..updateSessionIdHex('bad')
          ..updatePsm('0')
          ..updateDeviceId('');

    await viewModel.connect();

    expect(viewModel.error, contains('sessão'));
    expect(native.openedDeviceId, isNull);
    viewModel.dispose();
  });

  test('counts mismatched framed echoes as failures', () async {
    final native = FakeNativeLabService()..mismatchL2capEcho = true;
    final viewModel =
        L2capPayerLabViewModel(
            native,
            FakePermissionService(),
          )
          ..updateSessionIdHex('0123456789abcdef')
          ..updatePsm('25')
          ..updateDeviceId('AA:BB');
    await viewModel.connect();

    await viewModel.runEchoBenchmark();

    expect(viewModel.successCount, 0);
    expect(viewModel.error, isNotNull);
    await viewModel.close();
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  });
}
