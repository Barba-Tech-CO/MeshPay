import 'package:flutter/foundation.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/clock/percentile_helper.dart';

class ChannelLabViewModel extends ChangeNotifier {
  ChannelLabViewModel(this._nativeService);

  static const int _echoSizeBytes = 25 * 1024;
  static const int _echoIterations = 100;

  final NativeLabService _nativeService;
  int successCount = 0;
  double? p50Milliseconds;
  double? p95Milliseconds;
  bool isBusy = false;
  String? error;
  bool _isDisposed = false;

  Future<void> runEchoBenchmark() async {
    if (isBusy || _isDisposed) {
      return;
    }
    isBusy = true;
    successCount = 0;
    p50Milliseconds = null;
    p95Milliseconds = null;
    error = null;
    _notifyIfActive();
    try {
      _nativeService.setEchoHandler((data) async => data);
      final timingsNanos = await _nativeService.runLoopbackEcho(
        sizeBytes: _echoSizeBytes,
        iterations: _echoIterations,
      );
      successCount = timingsNanos.length;
      if (timingsNanos.isNotEmpty) {
        final timingsMilliseconds = timingsNanos
            .map((nanos) => nanos / 1000000)
            .toList(growable: false);
        p50Milliseconds = PercentileHelper.percentile(
          timingsMilliseconds,
          0.5,
        ).toDouble();
        p95Milliseconds = PercentileHelper.percentile(
          timingsMilliseconds,
          0.95,
        ).toDouble();
      }
    } on Object catch (failure) {
      error = failure.toString();
    } finally {
      isBusy = false;
      _notifyIfActive();
    }
  }

  void _notifyIfActive() {
    if (!_isDisposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    super.dispose();
  }
}
