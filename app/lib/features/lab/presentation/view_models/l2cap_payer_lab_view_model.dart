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
import '../../../../core/dtn/dtn_buffer_service.dart';
import '../../../../core/dtn/dtn_gossip_stats.dart';
import '../../../../core/dtn/payer_dtn_lab_flow.dart';
import '../../../../core/noise/noise_handshake_status.dart';
import '../../../../core/noise/noise_l2cap_session.dart';
import '../../../../core/noise/noise_service.dart';
import '../../../../core/tx/payer_tx_engine_service.dart';
import '../../../../core/tx/payer_tx_lab_flow.dart';
import '../../../../core/tx/tx_lab_state.dart';
import '../helpers/bluetooth_lab_helper.dart';
import '../helpers/l2cap_payer_echo_helper.dart';
import '../helpers/l2cap_payer_form.dart';
import '../helpers/l2cap_payer_input_helper.dart';
import '../helpers/l2cap_payer_scan_session.dart';
import '../helpers/l2cap_payer_session_opener.dart';
import '../helpers/lab_error_message_helper.dart';

class L2capPayerLabViewModel extends ChangeNotifier {
  L2capPayerLabViewModel(
    this._nativeService,
    this._permissionService,
    this._noiseService,
    this._payerTx,
    this._dtn,
  ) {
    _eventsSubscription = _nativeService.events.listen(
      handleNativeEvent,
      onError: handleNativeStreamError,
    );
    unawaited(_dtnFlow.refreshHeld().catchError(handleNativeStreamError));
  }

  final NativeLabService _nativeService;
  final PermissionService _permissionService;
  final NoiseService _noiseService;
  final PayerTxEngineService _payerTx;
  final DtnBufferService _dtn;
  late final StreamSubscription<NativeEvent> _eventsSubscription;
  late final L2capPayerScanSession _scan = L2capPayerScanSession(
    _nativeService,
  );
  late final PayerDtnLabFlow _dtnFlow = PayerDtnLabFlow(_dtn, _notifyIfActive);
  final L2capPayerForm _form = L2capPayerForm();
  NoiseL2capSession? _session;
  PayerTxLabFlow? _txFlow;
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

  DtnGossipStats get dtnStats => _dtnFlow.stats;

  String get sessionIdHex => _form.sessionIdHex;

  String get psm => _form.psm;

  String get keyHex => _form.keyHex;

  String get deviceId => _form.deviceId;

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

  void updateSessionIdHex(String value) => _form.sessionIdHex = value;

  void updatePsm(String value) => _form.psm = value;

  void updateKeyHex(String value) => _form.keyHex = value;

  void updateDeviceId(String value) => _form.deviceId = value;

  bool handleQrPayload(String payload) {
    error = _form.applyQr(payload);
    _notifyIfActive();
    return error == null;
  }

  Future<void> startScan() async {
    if (isScanning || isBusy || _isDisposed) {
      return;
    }
    error = await _scan.startChecked(
      sessionIdHex,
      _permissionService,
      () => _isDisposed,
    );
    _notifyIfActive();
  }

  Future<void> stopScan() async {
    if (!isScanning || _isDisposed) {
      return;
    }
    error = await _scan.stopChecked();
    _notifyIfActive();
  }

  void selectDevice(String selectedDeviceId) {
    _form.deviceId = selectedDeviceId;
    _notifyIfActive();
  }

  Future<void> connect() async {
    if (isBusy || isConnected || _isDisposed) {
      return;
    }
    final ({L2capPayerInput input, String deviceId}) target;
    try {
      target = _form.parseForConnect();
    } on FormatException catch (failure) {
      error = failure.message;
      _notifyIfActive();
      return;
    }
    error = await BluetoothLabHelper.permissionError(_permissionService);
    if (error != null || _isDisposed) {
      _notifyIfActive();
      return;
    }

    error = null;
    await _runBusy(() async {
      try {
        final session = await L2capPayerSessionOpener.open(
          nativeService: _nativeService,
          noiseService: _noiseService,
          input: target.input,
          deviceId: target.deviceId,
          beforeOpen: _scan.stopIfScanning,
        );
        _session = session;
        await _txFlow?.cancel();
        _txFlow = PayerTxLabFlow(
          session: session,
          engine: _payerTx,
          onChanged: _notifyIfActive,
        );
        _dtnFlow.startConnection();
        connectedDeviceId = target.deviceId;
        await _runHandshake(session);
      } on Object catch (failure) {
        error = LabErrorMessageHelper.messageFor(failure);
      }
    });
  }

  Future<void> runEchoBenchmark() => _runOnLink(
    'Conecte-se ao receptor antes de iniciar o eco.',
    (session) async {
      echoTimingsMilliseconds = const <double>[];
      final result = await session.measureEncryptedEchoes(
        body: L2capPayerEchoHelper.frame(),
        repetitions: L2capPayerEchoHelper.repetitions,
        shouldContinue: () => !_isDisposed && isConnected,
      );
      echoTimingsMilliseconds = result.timingsMilliseconds;
      if (result.failure case final failure?) {
        error ??= LabErrorMessageHelper.messageFor(failure);
      }
      return result.failure != null;
    },
    () => error,
  );

  Future<void> sendTestCheque() => _runTx((flow) => flow.sendCheque());

  Future<void> resendPending() => _runTx((flow) => flow.resendPending());

  Future<void> createTestEnvelope() async {
    if (isBusy || _isDisposed) {
      return;
    }
    await _runBusy(() async {
      await _dtnFlow.createTestEnvelope();
      error = _dtnFlow.error;
    });
  }

  Future<void> syncDtn() => _runOnLink(
    'Conecte-se ao receptor antes de sincronizar.',
    (session) => _dtnFlow.sync(session.exchangeEncrypted, session.peerPairKey),
    () => _dtnFlow.error,
  );

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
        if (_session case final session?
            when session.channelId == event.channelId) {
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
    return !isBusy &&
            isConnected &&
            !_isDisposed &&
            (session?.isUsable ?? false)
        ? session
        : null;
  }

  Future<void> _runTx(Future<bool> Function(PayerTxLabFlow flow) action) =>
      _runOnLink(
        'Conecte-se ao receptor antes de enviar o cheque.',
        (_) => action(_txFlow ?? (throw StateError('tx flow missing'))),
        () => _txFlow?.error,
      );

  /// Runs [action] on the usable session; it returns true when the link was
  /// lost, which tears the session down. [errorOf] gives the error to show.
  Future<void> _runOnLink(
    String notConnectedMessage,
    Future<bool> Function(NoiseL2capSession session) action,
    String? Function() errorOf,
  ) async {
    final session = _usableSession;
    if (session == null) {
      error = notConnectedMessage;
      _notifyIfActive();
      return;
    }
    error = null;
    await _runBusy(() async {
      final linkLost = await action(session);
      error = errorOf();
      if (linkLost) {
        await _teardown(session);
      }
    });
  }

  Future<void> _runBusy(Future<void> Function() action) async {
    isBusy = true;
    _notifyIfActive();
    try {
      await action();
    } finally {
      isBusy = false;
      _notifyIfActive();
    }
  }

  Future<void> _runHandshake(NoiseL2capSession session) async {
    try {
      handshakeDurationsMilliseconds = <double>[
        ...handshakeDurationsMilliseconds,
        await L2capPayerSessionOpener.timeHandshake(session),
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
