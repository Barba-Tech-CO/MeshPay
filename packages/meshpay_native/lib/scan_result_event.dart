import 'native_event.dart';

class ScanResultEvent extends NativeEvent {
  const ScanResultEvent({
    required this.deviceId,
    required this.rssi,
    required this.sessionIdHex,
    required this.elapsedRealtimeNanos,
  });

  final String deviceId;
  final int rssi;
  final String sessionIdHex;
  final int elapsedRealtimeNanos;
}
