import 'package:flutter/services.dart';

import 'attested_payment_key.dart';
import 'bluetooth_status.dart';
import 'clock_sample.dart';
import 'device_capabilities.dart';
import 'native_event.dart';
import 'native_event_decoder.dart';
import 'native_payload.dart';
import 'payment_key_exception.dart';
import 'payment_key_info.dart';
import 'single_use_key_batch.dart';

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

  /// Throws [PaymentKeyException] `strongbox_unavailable` when [requireStrongBox]
  /// is set and the device has none; never falls back silently.
  Future<AttestedPaymentKey> generateAttestedPaymentKey({
    required String alias,
    required Uint8List challenge,
    required bool requireStrongBox,
    bool singleUse = false,
  }) async {
    final Object? value = await _invokeKey<Object?>(
      'generateAttestedPaymentKey',
      <String, Object>{
        'alias': alias,
        'challenge': challenge,
        'requireStrongBox': requireStrongBox,
        'singleUse': singleUse,
      },
    );
    return AttestedPaymentKey.fromPlatform(value);
  }

  /// SHA256withECDSA, DER-encoded signature.
  Future<Uint8List> signWithPaymentKey({
    required String alias,
    required Uint8List data,
  }) async {
    final Object? value = await _invokeKey<Object?>(
      'signWithPaymentKey',
      <String, Object>{'alias': alias, 'data': data},
    );
    return NativePayload.readBytes(value, 'signature');
  }

  Future<void> deletePaymentKey({required String alias}) async {
    await _invokeKey<void>('deletePaymentKey', <String, Object>{
      'alias': alias,
    });
  }

  Future<PaymentKeyInfo> getPaymentKeyInfo({required String alias}) async {
    final Object? value = await _invokeKey<Object?>(
      'paymentKeyInfo',
      <String, Object>{'alias': alias},
    );
    return PaymentKeyInfo.fromPlatform(value);
  }

  Future<DeviceCapabilities> getDeviceCapabilities() async {
    final Object? value = await _invokeKey<Object?>('deviceCapabilities');
    return DeviceCapabilities.fromPlatform(value);
  }

  /// Generates [count] keys "$aliasPrefix0".."$aliasPrefix{count-1}" and times
  /// each one. Pass [challenge] to include attestation in the timing.
  Future<SingleUseKeyBatch> generateSingleUseKeys({
    required String aliasPrefix,
    required int count,
    required bool requireStrongBox,
    Uint8List? challenge,
    bool applyMaxUsageCount = true,
  }) async {
    final Object? value = await _invokeKey<Object?>(
      'generateSingleUseKeys',
      <String, Object?>{
        'aliasPrefix': aliasPrefix,
        'count': count,
        'requireStrongBox': requireStrongBox,
        'challenge': challenge,
        'applyMaxUsageCount': applyMaxUsageCount,
      },
    );
    return SingleUseKeyBatch.fromPlatform(value);
  }

  Future<T?> _invokeKey<T>(String method, [Object? arguments]) async {
    try {
      return await _methodChannel.invokeMethod<T>(method, arguments);
    } on PlatformException catch (error) {
      throw PaymentKeyException.fromPlatform(error);
    }
  }
}
