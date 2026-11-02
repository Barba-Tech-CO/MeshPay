import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_error_event.dart';
import 'package:meshpay_native/native_event.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import '../../../../core/clock/percentile_helper.dart';
import '../../../../core/noise/hex_helper.dart';
import '../../../../core/noise/noise_handshake_status.dart';
import '../../../../core/noise/noise_l2cap_session.dart';
import '../../../../core/noise/noise_prologue_helper.dart';
import '../../../../core/noise/noise_service.dart';
import '../../../../core/tx/lab_frame_helper.dart';
import '../../../../core/tx/payer_tx_engine_service.dart';
import '../../../../core/tx/payer_tx_lab_flow.dart';
import '../../../../core/tx/tx_lab_state.dart';
import '../helpers/bluetooth_lab_helper.dart';
import '../helpers/l2cap_payer_input_helper.dart';
import '../helpers/l2cap_payer_scan_session.dart';
import '../helpers/lab_error_message_helper.dart';

class L2capPayerLabViewModel extends ChangeNotifier {
  L2capPayerLabViewModel(
    this._nativeService,
    this._permissionService,
    this._noiseService,
    this._payerTx,
  ) {
    _eventsSubscription = _nativeService.events.listen(
      handleNativeEvent,
      onError: handleNativeStreamError,
    );
  }

  static const int _echoBodyBytes = 25 * 1024;
  static const int _echoRepetitions = 5;

  final NativeLabService _nativeService;
  final PermissionService _permissionService;
  final NoiseService _noiseService;
  final PayerTxEngineService _payerTx;
  late final StreamSubscription<NativeEvent> _eventsSubscription;
  late final L2capPayerScanSession _scan = L2capPayerScanSession(
    _nativeService,
  );
  NoiseL2capSession? _session;
  PayerTxLabFlow? _txFlow;
  String sessionIdHex = '';
  String psm = '';
  String keyHex = '';
  String deviceId = '';
  String? connectedDeviceId;
  List<double> echoTimingsMilliseconds = const <double>[];
  List<double> handshakeDurationsMilliseconds = const <double>[];
  bool isConnected = false;
  bool isBusy = false;
  String? error;
  bool _isDisposed = false;

  int get successCount => echoTimingsMilliseconds.length;

  double? get p50Milliseconds =>
      PercentileHelper.percentileOrNull(echoTimingsMilliseconds, 0.5);

  double? get p95Milliseconds =>
      PercentileHelper.percentileOrNull(echoTimingsMilliseconds, 0.95);

  TxLabState get txState => _txFlow?.state ?? TxLabState.idle;

  String? get txIdHex => _txFlow?.txIdHex;

  List<ScanResultEvent> get scanResults => _scan.results;

  bool get isScanning => _scan.isScanning;

  int? get connectedChannelId => isConnected ? _session?.channelId : null;

  NoiseHandshakeStatus get handshakeStatus =>
      _session?.status ?? NoiseHandshakeStatus.idle;

  List<int> get handshakeMessageSizes =>
      _session?.messageSizes ?? const <int>[];

  int get handshakeTotalBytes =>
      handshakeMessageSizes.fold<int>(0, (total, size) => total + size);

  double? get handshakeP50Milliseconds =>
      PercentileHelper.percentileOrNull(handshakeDurationsMilliseconds, 0.5);

  double? get handshakeP95Milliseconds =>
      PercentileHelper.percentileOrNull(handshakeDurationsMilliseconds, 0.95);

  void updateSessionIdHex(String value) {
    sessionIdHex = value;
  }

  void updatePsm(String value) {
    psm = value;
  }

  void updateKeyHex(String value) {
    keyHex = value;
  }

  void updateDeviceId(String value) {
    deviceId = value;
  }

  bool handleQrPayload(String payload) {
    error = null;
    try {
      final input = L2capPayerInputHelper.parseQr(payload);
      sessionIdHex = input.sessionIdHex;
      psm = '${input.psm}';
      keyHex = input.keyHex;
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
    final currentSessionId = sessionIdHex.trim().toLowerCase();
    if (!L2capPayerInputHelper.isValidSessionId(currentSessionId)) {
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
      await _scan.start(currentSessionId);
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
      await _scan.stop();
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
    final L2capPayerInput input;
    try {
      input = L2capPayerInputHelper.parseManual(
        sessionIdHex: sessionIdHex,
        psm: psm,
        keyHex: keyHex,
      );
    } on FormatException catch (failure) {
      error = failure.message;
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
      final noise = _noiseService.newInitiator(
        responderPublicKey: HexHelper.decode(input.keyHex),
        prologue: NoisePrologueHelper.build(
          sessionIdHex: input.sessionIdHex,
          psm: input.psm,
          keyHex: input.keyHex,
        ),
      );
      if (isScanning) {
        await _scan.stop();
      }
      final channelId = await _nativeService.openL2cap(
        deviceId: targetDeviceId,
        psm: input.psm,
      );
      final session = NoiseL2capSession(
        nativeService: _nativeService,
        channelId: channelId,
        noise: noise,
      );
      _session = session;
      await _txFlow?.cancel();
      _txFlow = PayerTxLabFlow(
        session: session,
        engine: _payerTx,
        onChanged: _notifyIfActive,
      );
      connectedDeviceId = targetDeviceId;
      await _runHandshake(session);
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    } finally {
      isBusy = false;
      _notifyIfActive();
    }
  }

  Future<void> runEchoBenchmark() async {
    final session = _usableSession;
    if (session == null) {
      error = 'Conecte-se ao receptor antes de iniciar o eco.';
      _notifyIfActive();
      return;
    }
    isBusy = true;
    echoTimingsMilliseconds = const <double>[];
    error = null;
    _notifyIfActive();

    final body = LabFrameHelper.encode(
      LabFrameType.echo,
      Uint8List(_echoBodyBytes)..fillRange(0, _echoBodyBytes, 0x5a),
    );
    final result = await session.measureEncryptedEchoes(
      body: body,
      repetitions: _echoRepetitions,
      shouldContinue: () => !_isDisposed && isConnected,
    );
    echoTimingsMilliseconds = result.timingsMilliseconds;
    if (result.failure case final failure?) {
      error ??= LabErrorMessageHelper.messageFor(failure);
      await _teardown(session);
    }
    isBusy = false;
    _notifyIfActive();
  }

  Future<void> sendTestCheque() => _runTx((flow) => flow.sendCheque());

  Future<void> resendPending() => _runTx((flow) => flow.resendPending());

  Future<void> close() async {
    final session = _session;
    if (session == null) {
      return;
    }
    await _txFlow?.cancel();
    await _teardown(session);
    _notifyIfActive();
  }

  void handleNativeEvent(NativeEvent event) {
    switch (event) {
      case ScanResultEvent():
        _scan.record(event);
      case L2capOpenedEvent():
        break;
      case L2capDataEvent():
        _handleL2capData(event);
      case L2capClosedEvent():
        _handleL2capClosed(event.channelId);
      case NativeErrorEvent():
        final message = LabErrorMessageHelper.messageFor(
          PlatformException(code: event.code, message: event.message),
        );
        error = message;
        final session = _session;
        if (session != null && session.channelId == event.channelId) {
          session.failPending(StateError(message));
        }
    }
    _notifyIfActive();
  }

  void handleNativeStreamError(Object failure) {
    error = LabErrorMessageHelper.messageFor(failure);
    _notifyIfActive();
  }

  /// The established, open session when nothing else is running.
  NoiseL2capSession? get _usableSession {
    final session = _session;
    final isUsable =
        !isBusy &&
        isConnected &&
        !_isDisposed &&
        session != null &&
        !session.isClosed &&
        session.isEstablished;
    return isUsable ? session : null;
  }

  Future<void> _runTx(Future<bool> Function(PayerTxLabFlow flow) action) async {
    final session = _usableSession;
    final flow = _txFlow;
    if (session == null || flow == null) {
      error = 'Conecte-se ao receptor antes de enviar o cheque.';
      _notifyIfActive();
      return;
    }
    isBusy = true;
    error = null;
    _notifyIfActive();
    final linkLost = await action(flow);
    error = flow.error;
    if (linkLost) {
      await _teardown(session);
    }
    isBusy = false;
    _notifyIfActive();
  }

  Future<void> _runHandshake(NoiseL2capSession session) async {
    final stopwatch = Stopwatch()..start();
    try {
      await session.runInitiatorHandshake();
      stopwatch.stop();
      handshakeDurationsMilliseconds = <double>[
        ...handshakeDurationsMilliseconds,
        stopwatch.elapsedMicroseconds / 1000,
      ];
      isConnected = true;
    } on Object {
      session.markFailed();
      await _teardown(session);
      error = LabErrorMessageHelper.noiseHandshakeMessage;
    }
  }

  Future<void> _teardown(NoiseL2capSession session) async {
    isConnected = false;
    connectedDeviceId = null;
    try {
      await session.close();
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    }
  }

  void _handleL2capData(L2capDataEvent event) {
    final session = _session;
    if (session == null ||
        session.isClosed ||
        session.channelId != event.channelId) {
      return;
    }
    try {
      session.deliver(event.data);
    } on FormatException catch (failure) {
      error = 'Quadro inválido: ${failure.message}';
      session.failPending(StateError(error!));
      unawaited(_teardown(session));
    }
  }

  void _handleL2capClosed(int channelId) {
    final session = _session;
    if (session == null || session.channelId != channelId) {
      return;
    }
    session.onRemoteClosed();
    isConnected = false;
    connectedDeviceId = null;
  }

  Future<bool> _hasBluetoothPermissions() async {
    error = await BluetoothLabHelper.permissionError(_permissionService);
    return error == null;
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
    final session = _session;
    final cleanup = <Future<void>>[
      if (isScanning) _scan.stop(),
      if (session != null) session.close(),
      if (_txFlow case final flow?) flow.cancel(),
    ];
    if (cleanup.isNotEmpty) {
      unawaited(
        Future.wait<void>(cleanup)
            .then<void>((_) {})
            .catchError(BluetoothLabHelper.reportCleanupError),
      );
    }
    super.dispose();
  }
}
