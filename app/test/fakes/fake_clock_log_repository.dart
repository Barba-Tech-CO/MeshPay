import 'package:meshpay/core/clock/clock_log_entry.dart';
import 'package:meshpay/core/clock/clock_log_repository.dart';
import 'package:meshpay_native/clock_sample.dart';

class FakeClockLogRepository implements ClockLogRepository {
  final List<ClockLogEntry> entries = <ClockLogEntry>[];
  bool shouldFail = false;

  @override
  Future<ClockLogEntry> append({
    required ClockSample sample,
    required String label,
    required bool isAppLaunch,
  }) async {
    if (shouldFail) {
      throw StateError('clock log unavailable');
    }
    final entry = ClockLogEntry(
      sequence: entries.length + 1,
      elapsedRealtimeNanos: sample.elapsedRealtimeNanos,
      bootCount: sample.bootCount,
      label: label,
      isAppLaunch: isAppLaunch,
    );
    entries.add(entry);
    return entry;
  }

  @override
  Future<List<ClockLogEntry>> readAll() async =>
      List<ClockLogEntry>.of(entries);
}
