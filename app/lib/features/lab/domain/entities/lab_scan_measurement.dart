import 'package:meshpay_native/scan_result_event.dart';

class LabScanMeasurement {
  const LabScanMeasurement({
    required this.event,
    required this.firstSightingMilliseconds,
  });

  final ScanResultEvent event;
  final int firstSightingMilliseconds;

  String get deviceId => event.deviceId;

  int get rssi => event.rssi;

  String get sessionIdHex => event.sessionIdHex;
}
