import 'native_payload.dart';

class ClockSample {
  const ClockSample({
    required this.elapsedRealtimeNanos,
    required this.bootCount,
  });

  final int elapsedRealtimeNanos;
  final int? bootCount;

  factory ClockSample.fromPlatform(Object? value) {
    final payload = NativePayload.readMap(value, 'clock sample');
    return ClockSample(
      elapsedRealtimeNanos: NativePayload.readIntField(
        payload,
        'elapsedRealtimeNanos',
      ),
      bootCount: NativePayload.readOptionalInt(payload, 'bootCount'),
    );
  }
}
