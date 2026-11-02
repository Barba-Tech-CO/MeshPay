import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';
import 'package:meshpay_native/native_event.dart';

import '../../../../core/ble/l2cap_frame_codec.dart';
import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import '../helpers/bluetooth_lab_helper.dart';
import '../helpers/lab_error_message_helper.dart';

enum L2capReceiverStatus { closed, listening, connected }

class L2capReceiverLabViewModel extends ChangeNotifier {
  L2capReceiverLabViewModel(this._nativeService, this._permissionService) {
    _eventsSubscription = _nativeService.events.listen(
      handleNativeEvent,
      onError: handleNativeStreamError,
    );
    _nativeService.setEchoHandler(_echoReceivedData);
  }

  final NativeLabService _nativeService;
  final PermissionService _permissionService;
  late final StreamSubscription<NativeEvent> _eventsSubscription;
  final Set<int> _serverChannelIds = <int>{};
  L2capReceiverStatus status = L2capReceiverStatus.closed;
  String? sessionIdHex;
  int? psm;
  int receivedMessageCount = 0;
  bool isBusy = false;
  String? error;
  bool _isAdvertising = false;
  bool _isListening = false;
  bool _isDisposed = false;

  bool get hasActiveOperations =>
      _isAdvertising || _isListening || _serverChannelIds.isNotEmpty;

  String? get qrPayload {
    final currentPsm = psm;
    final currentSessionId = sessionIdHex;
    if (currentPsm == null || currentSessionId == null) {
      return null;
    }
    return jsonEncode(<String, Object>{
      'psm': currentPsm,
      'session': currentSessionId,
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
      status = L2capReceiverStatus.listening;
    } on Object catch (failure) {
      final failures = <Object>[failure];
      if (_isListening) {
        try {
          await _nativeService.stopListenL2cap();
          _isListening = false;
        } on Object catch (cleanupFailure) {
          failures.add(cleanupFailure);
        }
      }
      if (_isAdvertising) {
        try {
          await _nativeService.stopAdvertising();
          _isAdvertising = false;
        } on Object catch (cleanupFailure) {
          failures.add(cleanupFailure);
        }
      }
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
    for (final channelId in List<int>.of(_serverChannelIds)) {
      try {
        await _nativeService.closeL2cap(channelId: channelId);
        _serverChannelIds.remove(channelId);
      } on Object catch (failure) {
        failures.add(failure);
      }
    }

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
          _serverChannelIds.add(event.channelId);
          status = L2capReceiverStatus.connected;
        }
      case L2capClosedEvent():
        _serverChannelIds.remove(event.channelId);
        status = _statusForActiveChannels();
      case L2capDataEvent():
        break;
      default:
        break;
    }
    _notifyIfActive();
  }

  void handleNativeStreamError(Object failure) {
    error = LabErrorMessageHelper.messageFor(failure);
    _notifyIfActive();
  }

  Future<Uint8List> _echoReceivedData(Uint8List data) async {
    try {
      L2capFrameCodec.decode(data);
      if (!_isDisposed) {
        receivedMessageCount++;
        _notifyIfActive();
      }
    } on FormatException catch (failure) {
      if (!_isDisposed) {
        error = 'Quadro inválido: ${failure.message}';
        _notifyIfActive();
      }
    }
    return Uint8List.fromList(data);
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

  L2capReceiverStatus _statusForActiveChannels() {
    if (_serverChannelIds.isNotEmpty) {
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
    final cleanup = <Future<void>>[
      if (_isListening) _nativeService.stopListenL2cap(),
      if (_isAdvertising) _nativeService.stopAdvertising(),
      for (final channelId in _serverChannelIds)
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
        context: ErrorDescription('while stopping the L2CAP receiver'),
      ),
    );
  }
}
