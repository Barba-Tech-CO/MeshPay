import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/permission_handler_service.dart';
import 'package:meshpay_native/bluetooth_status.dart';
import 'package:permission_handler/permission_handler.dart';

import '../../fakes/fake_native_lab_service.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const methodChannel = MethodChannel(
    'flutter.baseflow.com/permissions/methods',
  );
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[];
  var requestStatus = 1;
  var checkStatus = 1;

  setUp(() {
    calls.clear();
    requestStatus = 1;
    checkStatus = 1;
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      calls.add(call);
      return switch (call.method) {
        'requestPermissions' => <int, int>{
          for (final permission in List<int>.from(call.arguments as List))
            permission: requestStatus,
        },
        'checkPermissionStatus' => checkStatus,
        _ => null,
      };
    });
  });

  tearDown(() {
    messenger.setMockMethodCallHandler(methodChannel, null);
  });

  test('requests location permission through Android SDK 30', () async {
    final native = FakeNativeLabService()
      ..bluetoothStatus = const BluetoothStatus(
        adapterEnabled: true,
        permissionsGranted: true,
        le2mPhySupported: true,
        leCodedPhySupported: true,
        leExtendedAdvertisingSupported: true,
        multipleAdvertisementSupported: true,
        sdkInt: 30,
      );
    final service = PermissionHandlerService(native);

    expect(await service.requestBluetoothPermissions(), isTrue);
    expect(calls.map((MethodCall call) => call.arguments), <Object?>[
      <int>[Permission.location.value],
    ]);
  });

  test(
    'checks all Bluetooth permissions on SDK 31 and reports denial',
    () async {
      final native = FakeNativeLabService()
        ..bluetoothStatus = const BluetoothStatus(
          adapterEnabled: true,
          permissionsGranted: true,
          le2mPhySupported: true,
          leCodedPhySupported: true,
          leExtendedAdvertisingSupported: true,
          multipleAdvertisementSupported: true,
          sdkInt: 31,
        );
      final service = PermissionHandlerService(native);
      checkStatus = 0;

      expect(await service.hasBluetoothPermissions(), isFalse);
      expect(calls.map((MethodCall call) => call.arguments), <Object?>[
        Permission.bluetoothScan.value,
        Permission.bluetoothAdvertise.value,
        Permission.bluetoothConnect.value,
      ]);
    },
  );
}
