import '../../../../core/ble/native_lab_service.dart';

/// Advertising and listening state of the receiver. The flags flip only after
/// the native call succeeds, so a failed stop can be retried.
class L2capReceiverRadio {
  L2capReceiverRadio(this._nativeService);

  static const int _minPsm = 1;
  static const int _maxPsm = 65535;

  final NativeLabService _nativeService;
  bool isAdvertising = false;
  bool isListening = false;

  bool get isActive => isAdvertising || isListening;

  /// Starts advertising [sessionIdHex] and listening; returns the PSM.
  Future<int> start(String sessionIdHex) async {
    await _nativeService.startAdvertising(sessionIdHex: sessionIdHex);
    isAdvertising = true;
    final psm = await _nativeService.listenL2cap();
    isListening = true;
    if (psm < _minPsm || psm > _maxPsm) {
      throw StateError('The native service returned an invalid L2CAP PSM.');
    }
    return psm;
  }

  Future<void> stop(List<Object> failures) async {
    if (isListening) {
      try {
        await _nativeService.stopListenL2cap();
        isListening = false;
      } on Object catch (failure) {
        failures.add(failure);
      }
    }
    if (isAdvertising) {
      try {
        await _nativeService.stopAdvertising();
        isAdvertising = false;
      } on Object catch (failure) {
        failures.add(failure);
      }
    }
  }

  /// Fire-and-forget stops used when the view model is disposed.
  List<Future<void>> stopForDispose() => <Future<void>>[
    if (isListening) _nativeService.stopListenL2cap(),
    if (isAdvertising) _nativeService.stopAdvertising(),
  ];
}
