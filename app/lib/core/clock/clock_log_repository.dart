import 'package:meshpay_native/clock_sample.dart';

import 'clock_log_entry.dart';

abstract interface class ClockLogRepository {
  Future<List<ClockLogEntry>> readAll();

  Future<ClockLogEntry> append({
    required ClockSample sample,
    required String label,
    required bool isAppLaunch,
  });
}
