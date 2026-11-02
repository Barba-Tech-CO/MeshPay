import 'package:meshpay_native/scan_result_event.dart';

import '../../../../core/ble/native_lab_service.dart';
import 'bluetooth_lab_helper.dart';

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
