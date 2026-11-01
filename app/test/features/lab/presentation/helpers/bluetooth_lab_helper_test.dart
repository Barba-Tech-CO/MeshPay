import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/helpers/bluetooth_lab_helper.dart';

void main() {
  test('rejects a sighting timestamp before the scan start', () {
    expect(
      () => BluetoothLabHelper.firstSightingMilliseconds(
        scanStartedAtNanos: 2,
        sightedAtNanos: 1,
      ),
      throwsStateError,
    );
  });

  test('reports native cleanup errors to Flutter', () {
    final originalOnError = FlutterError.onError;
    FlutterErrorDetails? reportedError;
    FlutterError.onError = (details) {
      reportedError = details;
    };
    addTearDown(() => FlutterError.onError = originalOnError);

    final error = StateError('cleanup failed');
    final stackTrace = StackTrace.current;
    BluetoothLabHelper.reportCleanupError(error, stackTrace);

    expect(reportedError?.exception, same(error));
    expect(reportedError?.stack, same(stackTrace));
    expect(reportedError?.context?.toDescription(), contains('Bluetooth lab'));
  });
}
