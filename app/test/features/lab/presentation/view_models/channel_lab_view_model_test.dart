import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/view_models/channel_lab_view_model.dart';

import '../../../../fakes/fake_native_lab_service.dart';

void main() {
  test('runs the native echo and computes p50 and p95 from timings', () async {
    final native = FakeNativeLabService()
      ..loopbackTimingsNanos = List<int>.generate(
        100,
        (index) => (index + 1) * 1000000,
      );
    final viewModel = ChannelLabViewModel(native);

    await viewModel.runEchoBenchmark();

    expect(native.echoHandlerRegistered, isTrue);
    expect(native.loopbackSizeBytes, 25 * 1024);
    expect(native.loopbackIterations, 100);
    expect(viewModel.successCount, 100);
    expect(viewModel.p50Milliseconds, 50);
    expect(viewModel.p95Milliseconds, 95);
    expect(viewModel.error, isNull);
  });

  test('counts a mismatched echo as a failure', () async {
    final native = FakeNativeLabService()
      ..loopbackTimingsNanos = List<int>.generate(
        99,
        (index) => (index + 1) * 1000000,
      );
    final viewModel = ChannelLabViewModel(native);

    await viewModel.runEchoBenchmark();

    expect(viewModel.successCount, 99);
    expect(viewModel.error, isNull);
  });

  test('reports native errors', () async {
    final native = FakeNativeLabService()
      ..loopbackError = StateError('native channel failed');
    final viewModel = ChannelLabViewModel(native);

    await viewModel.runEchoBenchmark();

    expect(viewModel.successCount, 0);
    expect(viewModel.error, contains('native channel failed'));
  });
}
