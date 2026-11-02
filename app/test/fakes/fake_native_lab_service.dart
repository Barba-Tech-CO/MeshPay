import 'dart:async';
import 'dart:typed_data';

import 'package:meshpay/core/ble/native_lab_service.dart';
import 'package:meshpay_native/bluetooth_status.dart';
import 'package:meshpay_native/clock_sample.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_error_event.dart';
import 'package:meshpay_native/native_event.dart';

class FakeNativeLabService implements NativeLabService {
  late final StreamController<NativeEvent> eventsController =
      StreamController<NativeEvent>.broadcast(
        onCancel: () {
          eventsCancelled = true;
        },
      );
  final List<ClockSample> clockSamples = <ClockSample>[];
  final List<int> closedChannels = <int>[];
  final List<Uint8List> sentL2capPayloads = <Uint8List>[];
  final Set<int> _clientChannelIds = <int>{};
  Object? clockError;
  Object? scanError;
  Object? advertisingError;
  Object? listeningError;
  Object? bluetoothStatusError;
  Object? sendL2capError;
  Object? loopbackError;
  int readClockCalls = 0;
  int startAdvertisingCalls = 0;
  int stopAdvertisingCalls = 0;
  int startScanCalls = 0;
  int stopScanCalls = 0;
  int stopListeningCalls = 0;
  int closeL2capCalls = 0;
  int? loopbackSizeBytes;
  int? loopbackIterations;
  String? advertisedSessionIdHex;
  String? scanSessionIdHex;
  String? openedDeviceId;
  int? openedPsm;
  BluetoothStatus bluetoothStatus = const BluetoothStatus(
    adapterEnabled: true,
    permissionsGranted: true,
    le2mPhySupported: true,
    leCodedPhySupported: true,
    leExtendedAdvertisingSupported: true,
    multipleAdvertisementSupported: true,
    sdkInt: 35,
  );
  List<int> loopbackTimingsNanos = <int>[];
  bool echoHandlerRegistered = false;
  bool mismatchL2capEcho = false;
  bool failL2capEchoWithNativeEvent = false;
  bool eventsCancelled = false;
  int _nextChannelId = 1;
  Future<Uint8List> Function(Uint8List data)? _echoHandler;
  Future<Uint8List?> Function(int channelId, Uint8List data)? l2capResponder;

  @override
  Stream<NativeEvent> get events => eventsController.stream;

  @override
  Future<ClockSample> readClock() {
    readClockCalls++;
    if (clockError case final error?) {
      return Future<ClockSample>.error(error);
    }
    if (clockSamples.isNotEmpty) {
      return Future<ClockSample>.value(clockSamples.removeAt(0));
    }
    return Future<ClockSample>.value(
      const ClockSample(elapsedRealtimeNanos: 1_000_000, bootCount: 1),
    );
  }

  @override
  Future<BluetoothStatus> getBluetoothStatus() async {
    if (bluetoothStatusError case final error?) {
      throw error;
    }
    return bluetoothStatus;
  }

  @override
  Future<void> startAdvertising({required String sessionIdHex}) async {
    startAdvertisingCalls++;
    advertisedSessionIdHex = sessionIdHex;
    if (advertisingError case final error?) {
      throw error;
    }
  }

  @override
  Future<void> stopAdvertising() async {
    stopAdvertisingCalls++;
  }

  @override
  Future<void> startScan({String? sessionIdHex}) async {
    startScanCalls++;
    scanSessionIdHex = sessionIdHex;
    if (scanError case final error?) {
      throw error;
    }
  }

  @override
  Future<void> stopScan() async {
    stopScanCalls++;
  }

  @override
  Future<int> listenL2cap() async {
    if (listeningError case final error?) {
      throw error;
    }
    return 25;
  }

  @override
  Future<void> stopListenL2cap() async {
    stopListeningCalls++;
  }

  @override
  Future<int> openL2cap({required String deviceId, required int psm}) async {
    final channelId = _nextChannelId++;
    openedDeviceId = deviceId;
    openedPsm = psm;
    _clientChannelIds.add(channelId);
    eventsController.add(
      L2capOpenedEvent(
        channelId: channelId,
        role: 'client',
        deviceId: deviceId,
      ),
    );
    return channelId;
  }

  @override
  Future<void> sendL2cap({
    required int channelId,
    required Uint8List data,
  }) async {
    sentL2capPayloads.add(Uint8List.fromList(data));
    if (sendL2capError case final error?) {
      throw error;
    }
    if (failL2capEchoWithNativeEvent) {
      eventsController.add(
        NativeErrorEvent(
          code: 'l2cap_io_error',
          message: 'A conexão foi encerrada.',
          channelId: channelId,
        ),
      );
      return;
    }
    if (!_clientChannelIds.contains(channelId)) {
      return;
    }
    final responder = l2capResponder;
    final response = responder == null
        ? Uint8List.fromList(data)
        : await responder(channelId, Uint8List.fromList(data));
    if (response == null) {
      return;
    }
    eventsController.add(
      L2capDataEvent(
        channelId: channelId,
        data: mismatchL2capEcho ? Uint8List(response.length) : response,
      ),
    );
  }

  @override
  Future<void> closeL2cap({required int channelId}) async {
    closeL2capCalls++;
    closedChannels.add(channelId);
  }

  @override
  void setEchoHandler(Future<Uint8List> Function(Uint8List data) handler) {
    echoHandlerRegistered = true;
    _echoHandler = handler;
  }

  Future<Uint8List> echoReceivedData(Uint8List data) async {
    final handler = _echoHandler;
    if (handler == null) {
      throw StateError('The echo handler has not been registered.');
    }
    return handler(Uint8List.fromList(data));
  }

  @override
  Future<List<int>> runLoopbackEcho({
    required int sizeBytes,
    required int iterations,
  }) async {
    loopbackSizeBytes = sizeBytes;
    loopbackIterations = iterations;
    if (loopbackError case final error?) {
      throw error;
    }
    final payload = Uint8List.fromList(
      List<int>.generate(sizeBytes, (index) => index & 0xff),
    );
    for (var iteration = 0; iteration < iterations; iteration++) {
      final response = await _echoHandler!(Uint8List.fromList(payload));
      if (response.length != sizeBytes ||
          response.asMap().entries.any(
            (entry) => entry.value != payload[entry.key],
          )) {
        throw StateError('Loopback echo mismatch');
      }
    }
    return loopbackTimingsNanos;
  }

  Future<void> cancelEvents() async {
    eventsCancelled = true;
    await eventsController.close();
  }
}
