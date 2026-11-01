import 'dart:typed_data';

import 'package:meshpay_native/bluetooth_status.dart';
import 'package:meshpay_native/clock_sample.dart';
import 'package:meshpay_native/native_event.dart';

abstract interface class NativeLabService {
  Stream<NativeEvent> get events;

  Future<ClockSample> readClock();

  Future<BluetoothStatus> getBluetoothStatus();

  Future<void> startAdvertising({required String sessionIdHex});

  Future<void> stopAdvertising();

  Future<void> startScan({String? sessionIdHex});

  Future<void> stopScan();

  Future<int> listenL2cap();

  Future<void> stopListenL2cap();

  Future<int> openL2cap({required String deviceId, required int psm});

  Future<void> sendL2cap({required int channelId, required Uint8List data});

  Future<void> closeL2cap({required int channelId});

  void setEchoHandler(Future<Uint8List> Function(Uint8List data) handler);

  Future<List<int>> runLoopbackEcho({
    required int sizeBytes,
    required int iterations,
  });
}
