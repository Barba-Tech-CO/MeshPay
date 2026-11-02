import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_error_event.dart';
import 'package:meshpay_native/native_event.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../core/ble/l2cap_frame_codec.dart';
import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import '../../../../core/clock/percentile_helper.dart';
import '../helpers/lab_error_message_helper.dart';

class L2capPayerLabViewModel extends ChangeNotifier {
  L2capPayerLabViewModel(this._nativeService, this._permissionService) {
    _eventsSubscription = _nativeService.events.listen(
      handleNativeEvent,
      onError: handleNativeStreamError,
    );
  }

  static const int _sessionIdHexLength = 16;
  static const int _echoBodyBytes = 25 * 1024;
  static const int _echoRepetitions = 5;
  static const Duration _echoTimeout = Duration(seconds: 5);

  final NativeLabService _nativeService;
  final PermissionService _permissionService;
  late final StreamSubscription<NativeEvent> _eventsSubscription;
  final Map<int, Completer<Uint8List>> _pendingEchoes =
      <int, Completer<Uint8List>>{};
  final Set<int> _activeChannelIds = <int>{};
  List<ScanResultEvent> scanResults = const <ScanResultEvent>[];
  String sessionIdHex = '';
  String psm = '';
  String deviceId = '';
  String? _scanSessionIdHex;
  int? connectedChannelId;
  String? connectedDeviceId;
  int successCount = 0;
  double? p50Milliseconds;
  double? p95Milliseconds;
  bool isScanning = false;
  bool isConnected = false;
  bool isBusy = false;
  String? error;
  bool _isDisposed = false;

  void updateSessionIdHex(String value) {
    sessionIdHex = value;
  }

  void updatePsm(String value) {
    psm = value;
  }

  void updateDeviceId(String value) {
    deviceId = value;
  }

  bool handleQrPayload(String payload) {
    error = null;
    try {
      final decoded = jsonDecode(payload);
      if (decoded is! Map<String, Object?>) {
        throw const FormatException('O conteúdo do QR não é um objeto.');
      }
      final parsedSessionId = decoded['session'];
      final parsedPsm = decoded['psm'];
      if (parsedSessionId is! String || !_isValidSessionId(parsedSessionId)) {
        throw const FormatException('O ID de sessão do QR é inválido.');
      }
      if (parsedPsm is! int || parsedPsm < 1 || parsedPsm > 65535) {
        throw const FormatException('O PSM do QR é inválido.');
      }
      sessionIdHex = parsedSessionId;
      psm = '$parsedPsm';
      _notifyIfActive();
      return true;
    } on FormatException catch (failure) {
      error = 'QR inválido: ${failure.message}';
      _notifyIfActive();
      return false;
    }
  }

  Future<void> startScan() async {
    if (isScanning || isBusy || _isDisposed) {
      return;
    }
    final currentSessionId = sessionIdHex.trim();
    if (!_isValidSessionId(currentSessionId)) {
      error = 'Informe um ID de sessão válido em hexadecimal (8 bytes).';
      _notifyIfActive();
      return;
    }
    if (!await _hasBluetoothPermissions()) {
      _notifyIfActive();
      return;
    }
    if (_isDisposed) {
      return;
    }

    error = null;
    try {
      await _nativeService.startScan(
        sessionIdHex: currentSessionId.toLowerCase(),
      );
      _scanSessionIdHex = currentSessionId.toLowerCase();
      scanResults = const <ScanResultEvent>[];
      isScanning = true;
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    }
    _notifyIfActive();
  }

  Future<void> stopScan() async {
    if (!isScanning || _isDisposed) {
      return;
    }
    error = null;
    try {
      await _nativeService.stopScan();
      isScanning = false;
      _scanSessionIdHex = null;
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    }
    _notifyIfActive();
  }

  void selectDevice(String selectedDeviceId) {
    deviceId = selectedDeviceId;
    _notifyIfActive();
  }

  Future<void> connect() async {
    if (isBusy || isConnected || _isDisposed) {
      return;
    }
    final currentSessionId = sessionIdHex.trim();
    if (!_isValidSessionId(currentSessionId)) {
      error = 'Informe um ID de sessão válido em hexadecimal (8 bytes).';
      _notifyIfActive();
      return;
    }
    final parsedPsm = int.tryParse(psm.trim());
    if (parsedPsm == null || parsedPsm < 1 || parsedPsm > 65535) {
      error = 'Informe um PSM válido entre 1 e 65535.';
      _notifyIfActive();
      return;
    }
    final targetDeviceId = deviceId.trim();
    if (targetDeviceId.isEmpty) {
      error = 'Informe o ID do dispositivo encontrado pelo scan.';
      _notifyIfActive();
      return;
    }
    if (!await _hasBluetoothPermissions()) {
      _notifyIfActive();
      return;
    }
    if (_isDisposed) {
      return;
    }

    isBusy = true;
    error = null;
    _notifyIfActive();
    try {
      if (isScanning) {
        await _nativeService.stopScan();
        isScanning = false;
        _scanSessionIdHex = null;
      }
      final channelId = await _nativeService.openL2cap(
        deviceId: targetDeviceId,
        psm: parsedPsm,
      );
      connectedChannelId = channelId;
      connectedDeviceId = targetDeviceId;
      _activeChannelIds.add(channelId);
      isConnected = true;
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    } finally {
      isBusy = false;
      _notifyIfActive();
    }
  }

  Future<void> runEchoBenchmark() async {
    final channelId = connectedChannelId;
    if (isBusy || channelId == null || !isConnected || _isDisposed) {
      error = 'Conecte-se ao receptor antes de iniciar o eco.';
      _notifyIfActive();
      return;
    }
    isBusy = true;
    successCount = 0;
    p50Milliseconds = null;
    p95Milliseconds = null;
    error = null;
    _notifyIfActive();

    final body = Uint8List(_echoBodyBytes)..fillRange(0, _echoBodyBytes, 0x5a);
    final frame = L2capFrameCodec.encode(body);
    final timings = <num>[];
    for (var repetition = 0; repetition < _echoRepetitions; repetition++) {
      if (_isDisposed || !isConnected) {
        break;
      }
      final pendingEcho = Completer<Uint8List>();
      _pendingEchoes[channelId] = pendingEcho;
      final echo = pendingEcho.future.timeout(_echoTimeout);
      final stopwatch = Stopwatch()..start();
      try {
        await _nativeService.sendL2cap(channelId: channelId, data: frame);
        final echoedBody = await echo;
        if (!listEquals(body, echoedBody)) {
          throw StateError('Eco incorreto no canal L2CAP $channelId.');
        }
        timings.add(stopwatch.elapsedMicroseconds / 1000);
        successCount++;
      } on Object catch (failure) {
        error = LabErrorMessageHelper.messageFor(failure);
      } finally {
        stopwatch.stop();
        _pendingEchoes.remove(channelId);
      }
    }
    if (timings.isNotEmpty) {
      p50Milliseconds = PercentileHelper.percentile(
        timings,
        0.5,
      ).toDouble();
      p95Milliseconds = PercentileHelper.percentile(
        timings,
        0.95,
      ).toDouble();
    }
    isBusy = false;
    _notifyIfActive();
  }

  Future<void> close() async {
    final channelId = connectedChannelId;
    if (channelId == null) {
      return;
    }
    final pendingEcho = _pendingEchoes.remove(channelId);
    if (pendingEcho != null && !pendingEcho.isCompleted) {
      pendingEcho.completeError(
        StateError('O canal L2CAP foi fechado antes do eco.'),
      );
    }
    try {
      await _nativeService.closeL2cap(channelId: channelId);
      connectedChannelId = null;
      connectedDeviceId = null;
      isConnected = false;
      _activeChannelIds.remove(channelId);
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    }
    _notifyIfActive();
  }

  void handleNativeEvent(NativeEvent event) {
    switch (event) {
      case ScanResultEvent():
        _recordScanResult(event);
      case L2capOpenedEvent():
        if (event.role == 'client') {
          _activeChannelIds.add(event.channelId);
        }
      case L2capDataEvent():
        _handleL2capData(event);
      case L2capClosedEvent():
        _handleL2capClosed(event.channelId);
      case NativeErrorEvent():
        final message = LabErrorMessageHelper.messageFor(
          PlatformException(code: event.code, message: event.message),
        );
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
    error = LabErrorMessageHelper.messageFor(failure);
    _notifyIfActive();
  }

  void _recordScanResult(ScanResultEvent event) {
    final scanSessionIdHex = _scanSessionIdHex;
    if (!isScanning ||
        scanSessionIdHex == null ||
        event.sessionIdHex.toLowerCase() != scanSessionIdHex) {
      return;
    }
    final updatedResults = List<ScanResultEvent>.of(scanResults);
    final previousIndex = updatedResults.indexWhere(
      (result) => result.deviceId == event.deviceId,
    );
    if (previousIndex < 0) {
      updatedResults.add(event);
    } else {
      updatedResults[previousIndex] = event;
    }
    scanResults = List<ScanResultEvent>.unmodifiable(updatedResults);
  }

  void _handleL2capData(L2capDataEvent event) {
    final pending = _pendingEchoes[event.channelId];
    if (pending == null || pending.isCompleted) {
      return;
    }
    try {
      pending.complete(L2capFrameCodec.decode(event.data));
    } on FormatException catch (failure) {
      pending.completeError(
        StateError('Quadro de eco inválido: ${failure.message}'),
      );
    }
  }

  void _handleL2capClosed(int channelId) {
    _activeChannelIds.remove(channelId);
    if (connectedChannelId == channelId) {
      connectedChannelId = null;
      connectedDeviceId = null;
      isConnected = false;
    }
    final pending = _pendingEchoes.remove(channelId);
    if (pending != null && !pending.isCompleted) {
      pending.completeError(
        StateError('O canal L2CAP foi fechado antes do eco.'),
      );
    }
  }

  Future<bool> _hasBluetoothPermissions() async {
    try {
      if (await _permissionService.hasBluetoothPermissions()) {
        return true;
      }
      error = 'Conceda as permissões de Bluetooth antes de continuar.';
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    }
    return false;
  }

  bool _isValidSessionId(String value) {
    if (value.length != _sessionIdHexLength) {
      return false;
    }
    return value.codeUnits.every(
      (codeUnit) =>
          (codeUnit >= 48 && codeUnit <= 57) ||
          (codeUnit >= 65 && codeUnit <= 70) ||
          (codeUnit >= 97 && codeUnit <= 102),
    );
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
    for (final pending in _pendingEchoes.values) {
      if (!pending.isCompleted) {
        pending.completeError(StateError('A tela do pagador foi fechada.'));
      }
    }
    final cleanup = <Future<void>>[
      if (isScanning) _nativeService.stopScan(),
      for (final channelId in _activeChannelIds)
        _nativeService.closeL2cap(channelId: channelId),
    ];
    if (cleanup.isNotEmpty) {
      unawaited(
        Future.wait<void>(cleanup)
            .then<void>((_) {})
            .catchError(_reportCleanupError),
      );
    }
    super.dispose();
  }

  void _reportCleanupError(Object failure, StackTrace stackTrace) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: failure,
        stack: stackTrace,
        context: ErrorDescription('while stopping the L2CAP payer'),
      ),
    );
  }
}
