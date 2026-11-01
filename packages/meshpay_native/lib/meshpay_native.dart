import 'package:flutter/services.dart';

import 'bluetooth_status.dart';
import 'clock_sample.dart';
import 'native_event.dart';
import 'native_event_decoder.dart';
import 'native_payload.dart';

class MeshpayNative {
  MeshpayNative({MethodChannel? methodChannel, EventChannel? eventChannel})
    : _methodChannel =
          methodChannel ?? const MethodChannel('meshpay_native/methods'),
      _eventChannel =
          eventChannel ?? const EventChannel('meshpay_native/events');

  final MethodChannel _methodChannel;
  final EventChannel _eventChannel;
  late final Stream<NativeEvent> _events = _eventChannel
      .receiveBroadcastStream()
      .map(NativeEventDecoder.decode)
      .asBroadcastStream();

  Future<ClockSample> readClock() async {
    final Object? value = await _methodChannel.invokeMethod<Object?>(
      'readClock',
    );
    return ClockSample.fromPlatform(value);
  }

  Future<BluetoothStatus> getBluetoothStatus() async {
    final Object? value = await _methodChannel.invokeMethod<Object?>(
      'getBluetoothStatus',
    );
    return BluetoothStatus.fromPlatform(value);
  }

  Future<void> startAdvertising({required String sessionIdHex}) async {
    await _methodChannel.invokeMethod<void>(
      'startAdvertising',
      <String, Object>{'sessionIdHex': sessionIdHex},
    );
  }

  Future<void> stopAdvertising() async {
    await _methodChannel.invokeMethod<void>('stopAdvertising');
  }

  Future<void> startScan({String? sessionIdHex}) async {
    await _methodChannel.invokeMethod<void>('startScan', <String, Object?>{
      'sessionIdHex': sessionIdHex,
    });
  }

  Future<void> stopScan() async {
    await _methodChannel.invokeMethod<void>('stopScan');
  }

  Future<int> listenL2cap() async {
    final Object? value = await _methodChannel.invokeMethod<Object?>(
      'listenL2cap',
    );
    return NativePayload.readInt(value, 'psm');
  }

  Future<void> stopListenL2cap() async {
    await _methodChannel.invokeMethod<void>('stopListenL2cap');
  }

  Future<int> openL2cap({required String deviceId, required int psm}) async {
    final Object? value = await _methodChannel.invokeMethod<Object?>(
      'openL2cap',
      <String, Object>{'deviceId': deviceId, 'psm': psm},
    );
    return NativePayload.readInt(value, 'channelId');
  }

  Future<void> sendL2cap({
    required int channelId,
    required Uint8List data,
  }) async {
    await _methodChannel.invokeMethod<void>('sendL2cap', <String, Object>{
      'channelId': channelId,
      'data': data,
    });
  }

  Future<void> closeL2cap({required int channelId}) async {
    await _methodChannel.invokeMethod<void>('closeL2cap', <String, Object>{
      'channelId': channelId,
    });
  }

  Stream<NativeEvent> get events => _events;

  void setEchoHandler(Future<Uint8List> Function(Uint8List data) handler) {
    _methodChannel.setMethodCallHandler((MethodCall call) async {
      if (call.method != 'echo') {
        throw MissingPluginException(
          'Unsupported native method: ${call.method}',
        );
      }
      return handler(NativePayload.readBytes(call.arguments, 'echo payload'));
    });
  }

  Future<List<int>> runLoopbackEcho({
    required int sizeBytes,
    required int iterations,
  }) async {
    final Object? value = await _methodChannel.invokeMethod<Object?>(
      'runLoopbackEcho',
      <String, Object>{'sizeBytes': sizeBytes, 'iterations': iterations},
    );
    return NativePayload.readIntList(value, 'loopback timings');
  }
}
