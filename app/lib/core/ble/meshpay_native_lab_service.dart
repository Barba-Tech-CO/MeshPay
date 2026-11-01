import 'dart:typed_data';

import 'package:meshpay_native/bluetooth_status.dart';
import 'package:meshpay_native/clock_sample.dart';
import 'package:meshpay_native/meshpay_native.dart';
import 'package:meshpay_native/native_event.dart';

import 'native_lab_service.dart';

class MeshpayNativeLabService implements NativeLabService {
  MeshpayNativeLabService(this._native);

  final MeshpayNative _native;

  @override
  Stream<NativeEvent> get events => _native.events;

  @override
  Future<ClockSample> readClock() {
    return _native.readClock();
  }

  @override
  Future<BluetoothStatus> getBluetoothStatus() => _native.getBluetoothStatus();

  @override
  Future<void> startAdvertising({required String sessionIdHex}) {
    return _native.startAdvertising(
      sessionIdHex: sessionIdHex,
    );
  }

  @override
  Future<void> stopAdvertising() => _native.stopAdvertising();

  @override
  Future<void> startScan({String? sessionIdHex}) {
    return _native.startScan(sessionIdHex: sessionIdHex);
  }

  @override
  Future<void> stopScan() => _native.stopScan();

  @override
  Future<int> listenL2cap() => _native.listenL2cap();

  @override
  Future<void> stopListenL2cap() => _native.stopListenL2cap();

  @override
  Future<int> openL2cap({required String deviceId, required int psm}) {
    return _native.openL2cap(
      deviceId: deviceId,
      psm: psm,
    );
  }

  @override
  Future<void> sendL2cap({required int channelId, required Uint8List data}) {
    return _native.sendL2cap(
      channelId: channelId,
      data: data,
    );
  }

  @override
  Future<void> closeL2cap({required int channelId}) {
    return _native.closeL2cap(channelId: channelId);
  }

  @override
  void setEchoHandler(Future<Uint8List> Function(Uint8List data) handler) {
    _native.setEchoHandler(handler);
  }

  @override
  Future<List<int>> runLoopbackEcho({
    required int sizeBytes,
    required int iterations,
  }) {
    return _native.runLoopbackEcho(
      sizeBytes: sizeBytes,
      iterations: iterations,
    );
  }
}
