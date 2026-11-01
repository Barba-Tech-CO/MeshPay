import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay_native/meshpay_native.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_error_event.dart';
import 'package:meshpay_native/native_protocol_exception.dart';
import 'package:meshpay_native/scan_result_event.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const methodChannel = MethodChannel('meshpay_native/methods');
  const eventChannel = EventChannel('meshpay_native/events');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[];
  late MeshpayNative plugin;

  setUp(() {
    calls.clear();
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      calls.add(call);
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
          'sdkInt': 30,
        },
        'listenL2cap' => 25,
        'openL2cap' => 9,
        'runLoopbackEcho' => <int>[10, 20],
        _ => null,
      };
    });
    plugin = MeshpayNative(
      methodChannel: methodChannel,
      eventChannel: eventChannel,
    );
  });

  tearDown(() {
    messenger.setMockMethodCallHandler(methodChannel, null);
    messenger.setMockStreamHandler(eventChannel, null);
  });

  test('uses the public method names and arguments', () async {
    final data = Uint8List.fromList(<int>[1, 2, 255]);

    await plugin.readClock();
    await plugin.getBluetoothStatus();
    await plugin.startAdvertising(sessionIdHex: '0123456789abcdef');
    await plugin.stopAdvertising();
    await plugin.startScan();
    await plugin.startScan(sessionIdHex: 'aabb');
    await plugin.stopScan();
    expect(await plugin.listenL2cap(), 25);
    await plugin.stopListenL2cap();
    expect(await plugin.openL2cap(deviceId: 'AA:BB', psm: 25), 9);
    await plugin.sendL2cap(channelId: 9, data: data);
    await plugin.closeL2cap(channelId: 9);
    expect(await plugin.runLoopbackEcho(sizeBytes: 25, iterations: 3), <int>[
      10,
      20,
    ]);

    expect(calls.map((MethodCall call) => call.method), <String>[
      'readClock',
      'getBluetoothStatus',
      'startAdvertising',
      'stopAdvertising',
      'startScan',
      'startScan',
      'stopScan',
      'listenL2cap',
      'stopListenL2cap',
      'openL2cap',
      'sendL2cap',
      'closeL2cap',
      'runLoopbackEcho',
    ]);
    expect(calls[2].arguments, <String, Object>{
      'sessionIdHex': '0123456789abcdef',
    });
    expect(calls[4].arguments, <String, Object?>{'sessionIdHex': null});
    expect(calls[5].arguments, <String, Object?>{'sessionIdHex': 'aabb'});
    expect(calls[9].arguments, <String, Object>{
      'deviceId': 'AA:BB',
      'psm': 25,
    });
    expect(calls[10].arguments, <String, Object>{'channelId': 9, 'data': data});
    expect(calls[11].arguments, <String, Object>{'channelId': 9});
    expect(calls[12].arguments, <String, Object>{
      'sizeBytes': 25,
      'iterations': 3,
    });
  });

  test('decodes clock and Bluetooth status into typed models', () async {
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      return switch (call.method) {
        'readClock' => <String, Object>{
          'elapsedRealtimeNanos': 123456,
          'bootCount': 4,
        },
        'getBluetoothStatus' => <String, Object>{
          'adapterEnabled': true,
          'permissionsGranted': false,
          'le2mPhySupported': true,
          'leCodedPhySupported': false,
          'leExtendedAdvertisingSupported': true,
          'multipleAdvertisementSupported': false,
          'sdkInt': 31,
        },
        _ => null,
      };
    });

    final clock = await plugin.readClock();
    final status = await plugin.getBluetoothStatus();

    expect(clock.elapsedRealtimeNanos, 123456);
    expect(clock.bootCount, 4);
    expect(status.adapterEnabled, isTrue);
    expect(status.permissionsGranted, isFalse);
    expect(status.le2mPhySupported, isTrue);
    expect(status.leCodedPhySupported, isFalse);
    expect(status.leExtendedAdvertisingSupported, isTrue);
    expect(status.multipleAdvertisementSupported, isFalse);
    expect(status.sdkInt, 31);
  });

  test('decodes every native event type', () async {
    final events = <Object>[
      <String, Object>{
        'type': 'scanResult',
        'deviceId': 'AA:BB',
        'rssi': -42,
        'sessionIdHex': 'aabb',
        'elapsedRealtimeNanos': 100,
      },
      <String, Object>{
        'type': 'l2capOpened',
        'channelId': 9,
        'role': 'server',
        'deviceId': 'AA:BB',
      },
      <String, Object>{
        'type': 'l2capData',
        'channelId': 9,
        'data': Uint8List.fromList(<int>[1, 2]),
      },
      <String, Object>{'type': 'l2capClosed', 'channelId': 9},
      <String, Object>{
        'type': 'error',
        'code': 'permission_denied',
        'message': 'Bluetooth permission is missing.',
      },
    ];
    messenger.setMockStreamHandler(
      eventChannel,
      MockStreamHandler.inline(
        onListen: (_, MockStreamHandlerEventSink sink) {
          for (final event in events) {
            sink.success(event);
          }
        },
      ),
    );

    final decoded = await plugin.events.take(5).toList();

    expect(decoded[0], isA<ScanResultEvent>());
    expect((decoded[0] as ScanResultEvent).sessionIdHex, 'aabb');
    expect(decoded[1], isA<L2capOpenedEvent>());
    expect((decoded[1] as L2capOpenedEvent).role, 'server');
    expect((decoded[1] as L2capOpenedEvent).channelId, 9);
    expect(decoded[2], isA<L2capDataEvent>());
    expect((decoded[2] as L2capDataEvent).channelId, 9);
    expect(
      (decoded[2] as L2capDataEvent).data,
      Uint8List.fromList(<int>[1, 2]),
    );
    expect(decoded[3], isA<L2capClosedEvent>());
    expect((decoded[3] as L2capClosedEvent).channelId, 9);
    expect(decoded[4], isA<NativeErrorEvent>());
    expect((decoded[4] as NativeErrorEvent).code, 'permission_denied');
  });

  test('rejects malformed event payloads with a typed error', () async {
    messenger.setMockStreamHandler(
      eventChannel,
      MockStreamHandler.inline(
        onListen: (_, MockStreamHandlerEventSink sink) {
          sink.success(<String, Object>{
            'type': 'scanResult',
            'deviceId': 'AA:BB',
            'rssi': 'not-an-integer',
            'sessionIdHex': 'aabb',
            'elapsedRealtimeNanos': 100,
          });
        },
      ),
    );

    await expectLater(
      plugin.events.first,
      throwsA(isA<NativeProtocolException>()),
    );
  });

  test('registers the Dart loopback echo handler', () async {
    final payload = Uint8List.fromList(<int>[3, 4, 5]);
    plugin.setEchoHandler((Uint8List data) async => data);

    final response = await messenger.handlePlatformMessage(
      methodChannel.name,
      const StandardMethodCodec().encodeMethodCall(MethodCall('echo', payload)),
      null,
    );

    expect(const StandardMethodCodec().decodeEnvelope(response!), payload);
  });
}
