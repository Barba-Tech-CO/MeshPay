import 'package:meshpay_native/scan_result_event.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import 'bluetooth_lab_helper.dart';
import 'l2cap_payer_input_helper.dart';
import 'lab_error_message_helper.dart';

/// Scan state of the payer page: results are kept only for the session ID
/// being scanned.
class L2capPayerScanSession {
  L2capPayerScanSession(this._nativeService);

  final NativeLabService _nativeService;
  List<ScanResultEvent> results = const <ScanResultEvent>[];
  bool isScanning = false;
  String? _sessionIdHex;

  Future<void> start(String sessionIdHex) async {
    await _nativeService.startScan(sessionIdHex: sessionIdHex);
    _sessionIdHex = sessionIdHex;
    results = const <ScanResultEvent>[];
    isScanning = true;
  }

  /// Validates the session ID and permissions, then scans. Returns the error
  /// text, or null; does nothing when [isCancelled] turns true meanwhile.
  Future<String?> startChecked(
    String rawSessionIdHex,
    PermissionService permissions,
    bool Function() isCancelled,
  ) async {
    final sessionIdHex = rawSessionIdHex.trim().toLowerCase();
    if (!L2capPayerInputHelper.isValidSessionId(sessionIdHex)) {
      return 'Informe um ID de sessão válido em hexadecimal (8 bytes).';
    }
    final permissionError = await BluetoothLabHelper.permissionError(
      permissions,
    );
    if (permissionError != null || isCancelled()) {
      return permissionError;
    }
    try {
      await start(sessionIdHex);
      return null;
    } on Object catch (failure) {
      return LabErrorMessageHelper.messageFor(failure);
    }
  }

  /// Returns the error text, or null.
  Future<String?> stopChecked() async {
    try {
      await stop();
      return null;
    } on Object catch (failure) {
      return LabErrorMessageHelper.messageFor(failure);
    }
  }

  Future<void> stopIfScanning() async {
    if (isScanning) {
      await stop();
    }
  }

  Future<void> stop() async {
    await _nativeService.stopScan();
    isScanning = false;
    _sessionIdHex = null;
  }

  void record(ScanResultEvent event) {
    final scanned = _sessionIdHex;
    if (!isScanning ||
        scanned == null ||
        event.sessionIdHex.toLowerCase() != scanned) {
      return;
    }
    results = BluetoothLabHelper.upsertScanResult(results, event);
  }
}
