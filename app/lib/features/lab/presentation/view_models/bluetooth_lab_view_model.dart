import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:meshpay_native/bluetooth_status.dart';
import 'package:meshpay_native/clock_sample.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_error_event.dart';
import 'package:meshpay_native/native_event.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import '../../../../core/clock/percentile_helper.dart';
import '../../domain/entities/lab_scan_measurement.dart';
import '../helpers/bluetooth_lab_helper.dart';

class BluetoothLabViewModel extends ChangeNotifier {
  BluetoothLabViewModel(this._nativeService, this._permissionService) {
    _eventsSubscription = _nativeService.events.listen(
      handleNativeEvent,
      onError: handleNativeStreamError,
    );
  }

  static const int _echoSizeBytes = 1024;
  static const int _echoIterations = 20;
  static const Duration _echoTimeout = Duration(seconds: 5);

  final NativeLabService _nativeService;
  final PermissionService _permissionService;
  late final StreamSubscription<NativeEvent> _eventsSubscription;
  final Map<String, int> _firstSightings = <String, int>{};
  final Map<int, Completer<Uint8List>> _pendingEchoes =
      <int, Completer<Uint8List>>{};
  final Set<int> _activeChannelIds = <int>{};
  final Set<int> _serverChannelIds = <int>{};
  List<LabScanMeasurement> scanResults = const <LabScanMeasurement>[];
  BluetoothStatus? status;
  bool? permissionsGranted;
  bool isAdvertising = false;
  bool isScanning = false;
  bool isListening = false;
  bool isBusy = false;
  String? sessionIdHex;
  String? scanFilter;
  String? selectedDeviceId;
  String? connectedDeviceId;
  int? l2capPsm;
  int? connectedChannelId;
  int l2capSuccessCount = 0;
  double? l2capP50Milliseconds;
  double? l2capP95Milliseconds;
  String? error;
  ClockSample? _scanStartClock;
  bool _isDisposed = false;

  Future<void> initialize() async {
    try {
      status = await _nativeService.getBluetoothStatus();
      permissionsGranted = await _permissionService.hasBluetoothPermissions();
      _notifyIfActive();
    } on Object catch (failure) {
      error = failure.toString();
      _notifyIfActive();
    }
  }

  Future<void> requestBluetoothPermissions() async {
    error = null;
    try {
      permissionsGranted = await _permissionService
          .requestBluetoothPermissions();
      status = await _nativeService.getBluetoothStatus();
      if (permissionsGranted != true) {
        error = 'As permissões de Bluetooth não foram concedidas.';
      }
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> startAdvertising() async {
    if (!await _ensurePermissions()) {
      return;
    }
    error = null;
    final session = BluetoothLabHelper.createSessionIdHex();
    try {
      await _nativeService.startAdvertising(sessionIdHex: session);
      sessionIdHex = session;
      isAdvertising = true;
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> stopAdvertising() async {
    error = null;
    try {
      await _nativeService.stopAdvertising();
      isAdvertising = false;
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> startScan() async {
    if (!await _ensurePermissions()) {
      return;
    }
    error = null;
    try {
      final startClock = await _nativeService.readClock();
      await _nativeService.startScan(
        sessionIdHex: scanFilter?.trim().isEmpty == true
            ? null
            : scanFilter?.trim(),
      );
      _scanStartClock = startClock;
      _firstSightings.clear();
      scanResults = const <LabScanMeasurement>[];
      isScanning = true;
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> stopScan() async {
    error = null;
    try {
      await _nativeService.stopScan();
      isScanning = false;
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> startL2capListening() async {
    if (!await _ensurePermissions()) {
      return;
    }
    error = null;
    try {
      l2capPsm = await _nativeService.listenL2cap();
      isListening = true;
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> stopL2capListening() async {
    error = null;
    try {
      await _nativeService.stopListenL2cap();
      isListening = false;
      l2capPsm = null;
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> connectL2cap({
    required String deviceId,
    required String psm,
  }) async {
    if (!await _ensurePermissions()) {
      return;
    }
    final targetDeviceId = deviceId.trim().isEmpty
        ? selectedDeviceId
        : deviceId.trim();
    final targetPsm = int.tryParse(psm.trim());
    if (targetDeviceId == null ||
        targetDeviceId.isEmpty ||
        targetPsm == null ||
        targetPsm < 1 ||
        targetPsm > 65535) {
      error = 'Informe um dispositivo e um PSM válido.';
      _notifyIfActive();
      return;
    }

    error = null;
    try {
      final channelId = await _nativeService.openL2cap(
        deviceId: targetDeviceId,
        psm: targetPsm,
      );
      connectedDeviceId = targetDeviceId;
      connectedChannelId = channelId;
      _activeChannelIds.add(channelId);
    } on Object catch (failure) {
      error = failure.toString();
    }
    _notifyIfActive();
  }

  Future<void> runL2capEcho({
    required String deviceId,
    required String psm,
  }) async {
    if (!await _ensurePermissions()) {
      return;
    }
    final targetDeviceId = deviceId.trim().isEmpty
        ? selectedDeviceId
        : deviceId.trim();
    final targetPsm = int.tryParse(psm.trim());
    if (targetDeviceId == null ||
        targetDeviceId.isEmpty ||
        targetPsm == null ||
        targetPsm < 1 ||
        targetPsm > 65535) {
      error = 'Informe um dispositivo e um PSM válido.';
      _notifyIfActive();
      return;
    }

    isBusy = true;
    error = null;
    l2capSuccessCount = 0;
    l2capP50Milliseconds = null;
    l2capP95Milliseconds = null;
    _notifyIfActive();
    final durations = <num>[];
    for (var iteration = 0; iteration < _echoIterations; iteration++) {
      if (_isDisposed) {
        break;
      }
      try {
        durations.add(await _measureEchoAttempt(targetDeviceId, targetPsm));
        l2capSuccessCount++;
      } on Object catch (failure) {
        error = failure.toString();
      }
    }
    if (durations.isNotEmpty) {
      l2capP50Milliseconds = PercentileHelper.percentile(
        durations,
        0.5,
      ).toDouble();
      l2capP95Milliseconds = PercentileHelper.percentile(
        durations,
        0.95,
      ).toDouble();
    }
    isBusy = false;
    _notifyIfActive();
  }

  void updateScanFilter(String value) {
    scanFilter = value;
  }

  void selectScanResult(LabScanMeasurement result) {
    selectedDeviceId = result.deviceId;
    _notifyIfActive();
  }

  void handleNativeEvent(NativeEvent event) {
    switch (event) {
      case ScanResultEvent():
        _recordScanResult(event);
      case L2capOpenedEvent():
        _activeChannelIds.add(event.channelId);
        if (event.role == 'server') {
          _serverChannelIds.add(event.channelId);
        }
      case L2capDataEvent():
        _handleL2capData(event);
      case L2capClosedEvent():
        _activeChannelIds.remove(event.channelId);
        _serverChannelIds.remove(event.channelId);
        final pending = _pendingEchoes.remove(event.channelId);
        if (pending != null && !pending.isCompleted) {
          pending.completeError(
            StateError('O canal L2CAP foi fechado antes do eco.'),
          );
        }
      case NativeErrorEvent():
        final message = '${event.code}: ${event.message}';
        error = message;
        if (event.channelId case final channelId?) {
          final pending = _pendingEchoes.remove(channelId);
          if (pending != null && !pending.isCompleted) {
            pending.completeError(StateError(message));
          }
        }
    }
    _notifyIfActive();
  }

  void handleNativeStreamError(Object failure) {
    error = failure.toString();
    _notifyIfActive();
  }

  Future<double> _measureEchoAttempt(String deviceId, int psm) async {
    final stopwatch = Stopwatch()..start();
    int? channelId;
    try {
      channelId = await _nativeService.openL2cap(deviceId: deviceId, psm: psm);
      _activeChannelIds.add(channelId);
      final response = Completer<Uint8List>();
      _pendingEchoes[channelId] = response;
      final payload = Uint8List(_echoSizeBytes)
        ..fillRange(0, _echoSizeBytes, 0x5a);
      await _nativeService.sendL2cap(channelId: channelId, data: payload);
      final echoed = await response.future.timeout(_echoTimeout);
      if (!BluetoothLabHelper.bytesMatch(payload, echoed)) {
        throw StateError('Eco incorreto no canal L2CAP $channelId.');
      }
      return stopwatch.elapsedMicroseconds / 1000;
    } finally {
      stopwatch.stop();
      if (channelId != null) {
        _pendingEchoes.remove(channelId);
        _activeChannelIds.remove(channelId);
        await _nativeService.closeL2cap(channelId: channelId);
      }
    }
  }

  void _recordScanResult(ScanResultEvent event) {
    final scanStartClock = _scanStartClock;
    if (scanStartClock == null) {
      return;
    }
    try {
      final sessionId = event.sessionIdHex.toLowerCase();
      final milliseconds = BluetoothLabHelper.firstSightingMilliseconds(
        scanStartedAtNanos: scanStartClock.elapsedRealtimeNanos,
        sightedAtNanos: event.elapsedRealtimeNanos,
      );
      final firstSighting = _firstSightings.putIfAbsent(
        sessionId,
        () => milliseconds,
      );
      final measurement = LabScanMeasurement(
        event: event,
        firstSightingMilliseconds: firstSighting,
      );
      final updatedResults = List<LabScanMeasurement>.of(scanResults);
      final previousIndex = updatedResults.indexWhere(
        (result) => result.deviceId == event.deviceId,
      );
      if (previousIndex < 0) {
        updatedResults.add(measurement);
      } else {
        updatedResults[previousIndex] = measurement;
      }
      scanResults = List<LabScanMeasurement>.unmodifiable(updatedResults);
    } on Object catch (failure) {
      error = failure.toString();
    }
  }

  void _handleL2capData(L2capDataEvent event) {
    if (_serverChannelIds.contains(event.channelId)) {
      unawaited(_echoServerData(event));
      return;
    }
    final pending = _pendingEchoes[event.channelId];
    if (pending != null && !pending.isCompleted) {
      pending.complete(Uint8List.fromList(event.data));
    }
  }

  Future<void> _echoServerData(L2capDataEvent event) async {
    try {
      await _nativeService.sendL2cap(
        channelId: event.channelId,
        data: Uint8List.fromList(event.data),
      );
    } on Object catch (failure) {
      error = failure.toString();
      _notifyIfActive();
    }
  }

  Future<bool> _ensurePermissions() async {
    if (_isDisposed) {
      return false;
    }
    try {
      permissionsGranted = await _permissionService.hasBluetoothPermissions();
    } on Object catch (failure) {
      error = failure.toString();
      _notifyIfActive();
      return false;
    }
    if (_isDisposed) {
      return false;
    }
    if (permissionsGranted != true) {
      error = 'Conceda as permissões de Bluetooth antes de continuar.';
      _notifyIfActive();
      return false;
    }
    return true;
  }

  void _notifyIfActive() {
    if (!_isDisposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    unawaited(_eventsSubscription.cancel());
    unawaited(
      BluetoothLabHelper.stopNativeOperations(
        nativeService: _nativeService,
        channelIds: List<int>.of(_activeChannelIds),
      ).catchError(BluetoothLabHelper.reportCleanupError),
    );
    super.dispose();
  }
}
