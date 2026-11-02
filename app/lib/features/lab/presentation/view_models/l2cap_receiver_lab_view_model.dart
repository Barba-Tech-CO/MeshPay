import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_event.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import '../../../../core/noise/hex_helper.dart';
import '../../../../core/noise/noise_handshake_status.dart';
import '../../../../core/noise/noise_l2cap_session.dart';
import '../../../../core/noise/noise_prologue_helper.dart';
import '../../../../core/noise/noise_service.dart';
import '../helpers/bluetooth_lab_helper.dart';
import '../helpers/lab_error_message_helper.dart';

enum L2capReceiverStatus { closed, listening, connected }

/// One start = one charge: a single channel may complete one handshake, after
/// which the radio stops and the static key is wiped.
class L2capReceiverLabViewModel extends ChangeNotifier {
  L2capReceiverLabViewModel(
    this._nativeService,
    this._permissionService,
    this._noiseService,
  ) {
    _eventsSubscription = _nativeService.events.listen(
      handleNativeEvent,
      onError: handleNativeStreamError,
    );
  }

  final NativeLabService _nativeService;
  final PermissionService _permissionService;
  final NoiseService _noiseService;
  late final StreamSubscription<NativeEvent> _eventsSubscription;
  NoiseL2capSession? _session;
  int? _serverChannelId;
  Uint8List? _staticPrivateKey;
  Future<void> _frameQueue = Future<void>.value();
  L2capReceiverStatus status = L2capReceiverStatus.closed;
  String? sessionIdHex;
  int? psm;
  String? keyHex;
  int receivedMessageCount = 0;
  bool isBusy = false;
  String? error;
  bool _isAdvertising = false;
  bool _isListening = false;
  bool _isDisposed = false;

  bool get hasActiveOperations =>
      _isAdvertising || _isListening || _serverChannelId != null;

  NoiseHandshakeStatus get handshakeStatus =>
      _session?.status ?? NoiseHandshakeStatus.idle;

  List<int> get handshakeMessageSizes =>
      _session?.messageSizes ?? const <int>[];

  String? get qrPayload {
    final currentPsm = psm;
    final currentSessionId = sessionIdHex;
    final currentKey = keyHex;
    if (currentPsm == null || currentSessionId == null || currentKey == null) {
      return null;
    }
    return jsonEncode(<String, Object>{
      'psm': currentPsm,
      'session': currentSessionId,
      'key': currentKey,
    });
  }

  Future<void> start() async {
    if (isBusy ||
        status != L2capReceiverStatus.closed ||
        hasActiveOperations ||
        _isDisposed) {
      return;
    }
    isBusy = true;
    error = null;
    _notifyIfActive();

    if (!await _hasBluetoothPermissions()) {
      isBusy = false;
      _notifyIfActive();
      return;
    }
    if (_isDisposed) {
      return;
    }

    final newSessionId = BluetoothLabHelper.createSessionIdHex();
    final newKeypair = _noiseService.generateStaticKeypair();
    try {
      await _nativeService.startAdvertising(sessionIdHex: newSessionId);
      _isAdvertising = true;
      final newPsm = await _nativeService.listenL2cap();
      _isListening = true;
      if (newPsm < 1 || newPsm > 65535) {
        throw StateError('The native service returned an invalid L2CAP PSM.');
      }
      sessionIdHex = newSessionId;
      psm = newPsm;
      keyHex = HexHelper.encode(newKeypair.publicKey);
      _staticPrivateKey = newKeypair.privateKey;
      _session = null;
      status = L2capReceiverStatus.listening;
    } on Object catch (failure) {
      newKeypair.privateKey.fillRange(0, newKeypair.privateKey.length, 0);
      final failures = <Object>[failure];
      await _stopRadio(failures);
      status = _statusForActiveChannels();
      error = failures.map(LabErrorMessageHelper.messageFor).join('\n');
    } finally {
      isBusy = false;
      _notifyIfActive();
    }
  }

  Future<void> stop() async {
    if (isBusy || _isDisposed) {
      return;
    }
    isBusy = true;
    error = null;
    _notifyIfActive();
    final failures = <Object>[];

    await _stopRadio(failures);
    final session = _session;
    final channelId = _serverChannelId;
    try {
      if (session != null && !session.isClosed) {
        await session.close();
      } else if (channelId != null) {
        await _nativeService.closeL2cap(channelId: channelId);
      }
      _serverChannelId = null;
    } on Object catch (failure) {
      failures.add(failure);
    }
    _clearCharge();

    status = _statusForActiveChannels();
    if (failures.isNotEmpty) {
      error = failures.map(LabErrorMessageHelper.messageFor).join('\n');
    }
    isBusy = false;
    _notifyIfActive();
  }

  void handleNativeEvent(NativeEvent event) {
    switch (event) {
      case L2capOpenedEvent():
        if (event.role == 'server') {
          _acceptServerChannel(event.channelId);
        }
      case L2capClosedEvent():
        _handleChannelClosed(event.channelId);
      case L2capDataEvent():
        final session = _session;
        if (session != null &&
            !session.isClosed &&
            session.channelId == event.channelId) {
          _frameQueue = _frameQueue.then(
            (_) => _processData(session, event.data),
          );
        }
      default:
        break;
    }
    _notifyIfActive();
  }

  void handleNativeStreamError(Object failure) {
    error = LabErrorMessageHelper.messageFor(failure);
    _notifyIfActive();
  }

  void _acceptServerChannel(int channelId) {
    if (_serverChannelId != null) {
      unawaited(_closeChannel(channelId));
      return;
    }
    _serverChannelId = channelId;
    status = L2capReceiverStatus.connected;
    final currentSessionId = sessionIdHex;
    final currentPsm = psm;
    final currentKey = keyHex;
    final privateKey = _staticPrivateKey;
    if (currentSessionId == null ||
        currentPsm == null ||
        currentKey == null ||
        privateKey == null) {
      unawaited(_closeChannel(channelId));
      return;
    }
    try {
      _session = NoiseL2capSession(
        nativeService: _nativeService,
        channelId: channelId,
        noise: _noiseService.newResponder(
          localPrivateKey: privateKey,
          prologue: NoisePrologueHelper.build(
            sessionIdHex: currentSessionId,
            psm: currentPsm,
            keyHex: currentKey,
          ),
        ),
      );
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
      unawaited(_closeChannel(channelId));
    }
  }

  void _handleChannelClosed(int channelId) {
    if (_serverChannelId == channelId) {
      _serverChannelId = null;
    }
    final session = _session;
    if (session != null && session.channelId == channelId) {
      final wasInProgress = session.status == NoiseHandshakeStatus.inProgress;
      session.onRemoteClosed();
      if (wasInProgress) {
        unawaited(_endCharge());
      }
    }
    status = _statusForActiveChannels();
  }

  Future<void> _processData(NoiseL2capSession session, Uint8List data) async {
    if (!identical(_session, session) || session.isClosed || _isDisposed) {
      return;
    }
    var duringHandshake = !session.isEstablished;
    try {
      session.addData(data);
      for (var body = session.nextBody(); body != null;) {
        duringHandshake = !session.isEstablished;
        if (duringHandshake) {
          await session.respondHandshake(body);
          if (session.isEstablished) {
            await _endCharge();
          }
        } else {
          await session.echoEncrypted(body);
          receivedMessageCount++;
        }
        if (session.isClosed) {
          break;
        }
        body = session.nextBody();
      }
    } on FormatException catch (failure) {
      error = 'Quadro inválido: ${failure.message}';
      await _failSession(session, duringHandshake: duringHandshake);
    } on Object catch (failure) {
      error = duringHandshake
          ? LabErrorMessageHelper.noiseHandshakeMessage
          : LabErrorMessageHelper.messageFor(failure);
      await _failSession(session, duringHandshake: duringHandshake);
    }
    _notifyIfActive();
  }

  Future<void> _failSession(
    NoiseL2capSession session, {
    required bool duringHandshake,
  }) async {
    if (duringHandshake) {
      session.markFailed();
    }
    try {
      await session.close();
      if (_serverChannelId == session.channelId) {
        _serverChannelId = null;
      }
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    }
    if (duringHandshake) {
      await _endCharge();
    }
    status = _statusForActiveChannels();
  }

  Future<void> _closeChannel(int channelId) async {
    try {
      await _nativeService.closeL2cap(channelId: channelId);
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
      _notifyIfActive();
    }
  }

  Future<void> _stopRadio(List<Object> failures) async {
    if (_isListening) {
      try {
        await _nativeService.stopListenL2cap();
        _isListening = false;
      } on Object catch (failure) {
        failures.add(failure);
      }
    }
    if (_isAdvertising) {
      try {
        await _nativeService.stopAdvertising();
        _isAdvertising = false;
      } on Object catch (failure) {
        failures.add(failure);
      }
    }
  }

  Future<void> _endCharge() async {
    if (_staticPrivateKey == null) {
      return;
    }
    final failures = <Object>[];
    await _stopRadio(failures);
    _clearCharge();
    status = _statusForActiveChannels();
    if (failures.isNotEmpty) {
      error = failures.map(LabErrorMessageHelper.messageFor).join('\n');
    }
    _notifyIfActive();
  }

  void _clearCharge() {
    final privateKey = _staticPrivateKey;
    privateKey?.fillRange(0, privateKey.length, 0);
    _staticPrivateKey = null;
    sessionIdHex = null;
    psm = null;
    keyHex = null;
  }

  Future<bool> _hasBluetoothPermissions() async {
    error = await BluetoothLabHelper.permissionError(_permissionService);
    return error == null;
  }

  L2capReceiverStatus _statusForActiveChannels() {
    if (_serverChannelId != null) {
      return L2capReceiverStatus.connected;
    }
    return _isListening
        ? L2capReceiverStatus.listening
        : L2capReceiverStatus.closed;
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
    final channelId = _serverChannelId;
    final cleanup = <Future<void>>[
      if (_isListening) _nativeService.stopListenL2cap(),
      if (_isAdvertising) _nativeService.stopAdvertising(),
      if (session != null && !session.isClosed)
        session.close()
      else if (channelId != null)
        _nativeService.closeL2cap(channelId: channelId),
    ];
    _clearCharge();
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
