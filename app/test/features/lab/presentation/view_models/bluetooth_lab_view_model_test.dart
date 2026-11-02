import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/view_models/bluetooth_lab_view_model.dart';
import 'package:meshpay_native/clock_sample.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_permission_service.dart';

void main() {
  test('advertises a cryptographically random eight-byte session ID', () async {
    final native = FakeNativeLabService();
    final viewModel = BluetoothLabViewModel(native, FakePermissionService());

    await viewModel.startAdvertising();

    expect(viewModel.sessionIdHex, hasLength(16));
    expect(native.advertisedSessionIdHex, viewModel.sessionIdHex);
    expect(
      viewModel.sessionIdHex!.split('').every('0123456789abcdef'.contains),
      isTrue,
    );
  });

  test('requests permissions, scans, and measures first sightings', () async {
    final native = FakeNativeLabService()
      ..clockSamples.add(
        const ClockSample(elapsedRealtimeNanos: 1_000_000, bootCount: 1),
      );
    final permissions = FakePermissionService();
    final viewModel = BluetoothLabViewModel(native, permissions);

    await viewModel.requestBluetoothPermissions();
    await viewModel.startScan();
    native.eventsController.add(
      const ScanResultEvent(
        deviceId: 'AA:BB',
        rssi: -42,
        sessionIdHex: '0123456789abcdef',
        elapsedRealtimeNanos: 3_000_000,
      ),
    );
    await Future<void>.delayed(Duration.zero);

    expect(viewModel.permissionsGranted, isTrue);
    expect(viewModel.scanResults.single.deviceId, 'AA:BB');
    expect(viewModel.scanResults.single.firstSightingMilliseconds, 2);
  });

  test('does not start radio operations when permissions are denied', () async {
    final native = FakeNativeLabService();
    final permissions = FakePermissionService()..granted = false;
    final viewModel = BluetoothLabViewModel(native, permissions);

    await viewModel.startAdvertising();

    expect(viewModel.error, contains('permissões'));
    expect(native.startAdvertisingCalls, 0);
  });

  test(
    'stops radio operations and connects to the selected scan result',
    () async {
      final native = FakeNativeLabService()
        ..clockSamples.add(
          const ClockSample(elapsedRealtimeNanos: 1_000_000, bootCount: 1),
        );
      final viewModel = BluetoothLabViewModel(native, FakePermissionService());
      await viewModel.initialize();
      viewModel.updateScanFilter('0123456789abcdef');
      await viewModel.startScan();
      expect(native.scanSessionIdHex, '0123456789abcdef');

      viewModel.handleNativeEvent(
        const ScanResultEvent(
          deviceId: 'AA:BB',
          rssi: -30,
          sessionIdHex: '0123456789abcdef',
          elapsedRealtimeNanos: 2_000_000,
        ),
      );
      final result = viewModel.scanResults.single;
      expect(result.rssi, -30);
      expect(result.sessionIdHex, '0123456789abcdef');
      viewModel.selectScanResult(result);
      await viewModel.stopScan();
      await viewModel.startAdvertising();
      await viewModel.stopAdvertising();
      await viewModel.startL2capListening();
      await viewModel.connectL2cap(deviceId: '', psm: '25');

      expect(native.openedDeviceId, 'AA:BB');
      expect(native.openedPsm, 25);
      expect(viewModel.connectedDeviceId, 'AA:BB');
      final channelId = viewModel.connectedChannelId!;
      viewModel.handleNativeEvent(L2capClosedEvent(channelId: channelId));
      await viewModel.stopL2capListening();
      expect(viewModel.isScanning, isFalse);
      expect(viewModel.isAdvertising, isFalse);
      expect(viewModel.isListening, isFalse);
      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);
    },
  );

  test(
    'surfaces permission, status, native, and scan timestamp failures',
    () async {
      final native = FakeNativeLabService();
      final permissions = FakePermissionService()
        ..requestError = StateError('permission request failed');
      final viewModel = BluetoothLabViewModel(native, permissions);

      await viewModel.requestBluetoothPermissions();
      expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');

      permissions
        ..requestError = null
        ..checkError = StateError('permission status failed');
      await viewModel.startScan();
      expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');

      permissions.checkError = null;
      native.bluetoothStatusError = StateError('status unavailable');
      await viewModel.initialize();
      expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');

      native
        ..bluetoothStatusError = null
        ..scanError = StateError('scan failed');
      await viewModel.startScan();
      expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');

      native.scanError = null;
      await viewModel.startScan();
      viewModel.handleNativeEvent(
        const ScanResultEvent(
          deviceId: 'AA:BB',
          rssi: -50,
          sessionIdHex: 'fedcba9876543210',
          elapsedRealtimeNanos: 0,
        ),
      );
      expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');

      viewModel.handleNativeStreamError(StateError('event stream failed'));
      expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');
      await viewModel.connectL2cap(deviceId: '', psm: 'inválido');
      expect(viewModel.error, contains('PSM válido'));
      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);
    },
  );

  test('shows native error events and fails a pending L2CAP echo', () async {
    final native = FakeNativeLabService()..failL2capEchoWithNativeEvent = true;
    final viewModel = BluetoothLabViewModel(native, FakePermissionService());
    await viewModel.runL2capEcho(deviceId: 'AA:BB', psm: '25');

    expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');
    expect(viewModel.l2capSuccessCount, 0);
  });

  test(
    'measures 20 verified L2CAP echoes and closes channels on dispose',
    () async {
      final native = FakeNativeLabService();
      final viewModel = BluetoothLabViewModel(native, FakePermissionService());
      await viewModel.initialize();
      await viewModel.runL2capEcho(deviceId: 'AA:BB', psm: '25');

      expect(viewModel.l2capSuccessCount, 20);
      expect(viewModel.l2capP50Milliseconds, isNotNull);
      expect(native.closeL2capCalls, 20);

      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);

      expect(native.stopAdvertisingCalls, greaterThan(0));
      expect(native.stopScanCalls, greaterThan(0));
      expect(native.stopListeningCalls, greaterThan(0));
      expect(native.eventsCancelled, isTrue);
    },
  );

  test('counts mismatched L2CAP echoes as failures', () async {
    final native = FakeNativeLabService()..mismatchL2capEcho = true;
    final viewModel = BluetoothLabViewModel(native, FakePermissionService());
    await viewModel.initialize();
    await viewModel.runL2capEcho(deviceId: 'AA:BB', psm: '25');

    expect(viewModel.l2capSuccessCount, 0);
    expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');
  });

  test(
    'echoes server-role L2CAP data back through the native service',
    () async {
      final native = FakeNativeLabService();
      final viewModel = BluetoothLabViewModel(native, FakePermissionService());
      final payload = Uint8List.fromList(<int>[1, 2, 3]);
      viewModel.handleNativeEvent(
        const L2capOpenedEvent(
          channelId: 99,
          role: 'server',
          deviceId: 'AA:BB',
        ),
      );

      viewModel.handleNativeEvent(L2capDataEvent(channelId: 99, data: payload));
      await Future<void>.delayed(Duration.zero);

      expect(native.sentL2capPayloads.single, <int>[1, 2, 3]);
      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);
    },
  );
}
