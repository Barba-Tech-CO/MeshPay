import 'package:flutter/foundation.dart';
import 'package:meshpay_core/meshpay_core.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../ble/native_lab_service.dart';
import '../dtn/dtn_buffer_service.dart';
import '../server/trust/server_trust_service.dart';

/// Reads the monotonic clock and installs the held revocation snapshot (if
/// any) into the receiver engine, which anchors its protocol clock on it.
/// A fresh server snapshot wins when online; offline falls back to the held
/// one. Without a snapshot the engine refuses every new cheque.
class ReceiverClockHelper {
  ReceiverClockHelper._();

  static Future<ClockSample> prepare({
    required ReceiverTxEngine engine,
    required DtnBufferService dtn,
    required NativeLabService native,
    ServerTrustService? trust,
  }) async {
    final sample = await native.readClock();
    final fresh = await trust?.refreshSnapshot();
    if (fresh != null) {
      try {
        await _install(engine, fresh, sample);
        return sample;
      } on Object catch (error) {
        debugPrint('server snapshot refused: ${error.runtimeType}');
      }
    }
    final held = await dtn.snapshotBytes();
    if (held != null) {
      await _install(engine, held, sample);
    }
    return sample;
  }

  static Future<void> _install(
    ReceiverTxEngine engine,
    Uint8List snapshot,
    ClockSample sample,
  ) => engine.installSnapshot(
    snapshot: snapshot,
    monotonicNanos: BigInt.from(sample.elapsedRealtimeNanos),
    bootCount: bootCount(sample),
  );

  static BigInt? bootCount(ClockSample sample) {
    final count = sample.bootCount;
    return count == null ? null : BigInt.from(count);
  }
}
