import 'dart:math';

import 'package:flutter/foundation.dart';

import '../../../../core/ble/native_lab_service.dart';

class BluetoothLabHelper {
  BluetoothLabHelper._();

  static const int _nanosecondsPerMillisecond = 1000000;

  static String createSessionIdHex() {
    final random = Random.secure();
    return List<int>.generate(
      8,
      (_) => random.nextInt(256),
    ).map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
  }

  static int firstSightingMilliseconds({
    required int scanStartedAtNanos,
    required int sightedAtNanos,
  }) {
    final elapsedNanos = sightedAtNanos - scanStartedAtNanos;
    if (elapsedNanos < 0) {
      throw StateError('A descoberta precede o início do scan.');
    }
    return elapsedNanos ~/ _nanosecondsPerMillisecond;
  }

  static bool bytesMatch(Uint8List first, Uint8List second) =>
      listEquals(first, second);

  static Future<void> stopNativeOperations({
    required NativeLabService nativeService,
    required List<int> channelIds,
  }) async {
    await Future.wait<void>(<Future<void>>[
      nativeService.stopAdvertising(),
      nativeService.stopScan(),
      nativeService.stopListenL2cap(),
      for (final channelId in channelIds)
        nativeService.closeL2cap(channelId: channelId),
    ]);
  }

  static void reportCleanupError(Object error, StackTrace stackTrace) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: error,
        stack: stackTrace,
        context: ErrorDescription('while stopping Bluetooth lab operations'),
      ),
    );
  }
}
