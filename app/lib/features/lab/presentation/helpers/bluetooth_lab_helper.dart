import 'dart:math';

import 'package:flutter/foundation.dart';

import 'package:meshpay_native/scan_result_event.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/ble/permission_service.dart';
import 'lab_error_message_helper.dart';

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

  static List<ScanResultEvent> upsertScanResult(
    List<ScanResultEvent> results,
    ScanResultEvent event,
  ) {
    final updated = List<ScanResultEvent>.of(results);
    final index = updated.indexWhere(
      (result) => result.deviceId == event.deviceId,
    );
    if (index < 0) {
      updated.add(event);
    } else {
      updated[index] = event;
    }
    return List<ScanResultEvent>.unmodifiable(updated);
  }

  /// Null when permitted, otherwise the message to show the user.
  static Future<String?> permissionError(
    PermissionService permissionService,
  ) async {
    try {
      if (await permissionService.hasBluetoothPermissions()) {
        return null;
      }
      return 'Conceda as permissões de Bluetooth antes de continuar.';
    } on Object catch (failure) {
      return LabErrorMessageHelper.messageFor(failure);
    }
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
