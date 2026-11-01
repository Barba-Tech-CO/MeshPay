import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/meshpay_native_lab_service.dart';
import 'package:meshpay_native/meshpay_native.dart';
import 'package:meshpay_native/native_error_event.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const methodChannel = MethodChannel('meshpay_native/methods');
  const eventChannel = EventChannel('meshpay_native/events');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  late MeshpayNativeLabService service;

  setUp(() {
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      return switch (call.method) {
        'readClock' => <String, Object>{
          'elapsedRealtimeNanos': 123,
          'bootCount': 3,
        },
        'getBluetoothStatus' => <String, Object>{
          'adapterEnabled': true,
          'permissionsGranted': true,
          'le2mPhySupported': false,
          'leCodedPhySupported': true,
          'leExtendedAdvertisingSupported': true,
          'multipleAdvertisementSupported': true,
          'sdkInt': 35,
        },
        'listenL2cap' => 25,
        'openL2cap' => 9,
        'runLoopbackEcho' => <int>[10, 20],
        _ => null,
      };
    });
    service = MeshpayNativeLabService(
      MeshpayNative(methodChannel: methodChannel, eventChannel: eventChannel),
    );
  });

  tearDown(() {
    messenger.setMockMethodCallHandler(methodChannel, null);
    messenger.setMockStreamHandler(eventChannel, null);
  });

  test('delegates native operations and echo handler registration', () async {
    final payload = Uint8List.fromList(<int>[1, 2, 255]);

    expect((await service.readClock()).elapsedRealtimeNanos, 123);
    expect((await service.getBluetoothStatus()).sdkInt, 35);
    await service.startAdvertising(sessionIdHex: '0123456789abcdef');
    await service.stopAdvertising();
    await service.startScan(sessionIdHex: 'aabb');
    await service.stopScan();
    expect(await service.listenL2cap(), 25);
    await service.stopListenL2cap();
    expect(await service.openL2cap(deviceId: 'AA:BB', psm: 25), 9);
    await service.sendL2cap(channelId: 9, data: payload);
    await service.closeL2cap(channelId: 9);
    expect(await service.runLoopbackEcho(sizeBytes: 25, iterations: 3), <int>[
      10,
      20,
    ]);

    service.setEchoHandler((Uint8List data) async => data);
    final response = await messenger.handlePlatformMessage(
      methodChannel.name,
      const StandardMethodCodec().encodeMethodCall(MethodCall('echo', payload)),
      null,
    );

    expect(const StandardMethodCodec().decodeEnvelope(response!), payload);
  });

  test('exposes native events', () async {
    messenger.setMockStreamHandler(
      eventChannel,
      MockStreamHandler.inline(
        onListen: (_, MockStreamHandlerEventSink sink) {
          sink.success(<String, Object>{
            'type': 'error',
            'code': 'permission_denied',
            'message': 'Bluetooth permission is missing.',
          });
        },
      ),
    );

    final event = await service.events.first;

    expect(event, isA<NativeErrorEvent>());
  });
}
