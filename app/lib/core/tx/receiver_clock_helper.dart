import 'package:meshpay_core/meshpay_core.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../ble/native_lab_service.dart';
import '../dtn/dtn_buffer_service.dart';

/// Reads the monotonic clock and installs the held revocation snapshot (if
/// any) into the receiver engine, which anchors its protocol clock on it.
/// Without a snapshot the engine refuses every new cheque.
class ReceiverClockHelper {
  ReceiverClockHelper._();

  static Future<ClockSample> prepare({
    required ReceiverTxEngine engine,
    required DtnBufferService dtn,
    required NativeLabService native,
  }) async {
    final sample = await native.readClock();
    final snapshot = await dtn.snapshotBytes();
    if (snapshot != null) {
      await engine.installSnapshot(
        snapshot: snapshot,
        monotonicNanos: BigInt.from(sample.elapsedRealtimeNanos),
        bootCount: bootCount(sample),
      );
    }
    return sample;
  }

  static BigInt? bootCount(ClockSample sample) {
    final count = sample.bootCount;
    return count == null ? null : BigInt.from(count);
  }
}
