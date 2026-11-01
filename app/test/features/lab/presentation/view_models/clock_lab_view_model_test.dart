import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/view_models/clock_lab_view_model.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../../../../fakes/fake_clock_log_repository.dart';
import '../../../../fakes/fake_native_lab_service.dart';

void main() {
  test('records an initial sample and a user-requested sample', () async {
    final native = FakeNativeLabService()
      ..clockSamples.addAll(<ClockSample>[
        const ClockSample(elapsedRealtimeNanos: 100, bootCount: 2),
        const ClockSample(elapsedRealtimeNanos: 200, bootCount: 2),
      ]);
    final repository = FakeClockLogRepository();
    final viewModel = ClockLabViewModel(native, repository);

    await viewModel.initialize();
    await viewModel.readSample();

    expect(native.readClockCalls, 2);
    expect(repository.entries, hasLength(2));
    expect(repository.entries.first.label, 'Abertura do laboratório');
    expect(repository.entries.last.label, 'Amostra manual');
    expect(viewModel.entries.first.elapsedRealtimeNanos, 200);
  });

  test('exposes native clock failures to the page', () async {
    final native = FakeNativeLabService()
      ..clockError = StateError('clock offline');
    final viewModel = ClockLabViewModel(native, FakeClockLogRepository());

    await viewModel.initialize();

    expect(viewModel.error, contains('clock offline'));
    expect(viewModel.entries, isEmpty);
  });
}
