import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/pages/clock_lab_page.dart';
import 'package:meshpay/features/lab/presentation/view_models/clock_lab_view_model.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../../../../fakes/fake_clock_log_repository.dart';
import '../../../../fakes/fake_native_lab_service.dart';

void main() {
  testWidgets('shows clock entries, adds samples, and surfaces read errors', (
    tester,
  ) async {
    final native = FakeNativeLabService()
      ..clockSamples.addAll(<ClockSample>[
        const ClockSample(elapsedRealtimeNanos: 100, bootCount: 2),
        const ClockSample(elapsedRealtimeNanos: 200, bootCount: 3),
      ]);
    final viewModel = ClockLabViewModel(native, FakeClockLogRepository());
    final module = createModule(register: (_) {});

    await tester.pumpWidget(
      ModularApp(
        module: module,
        provide: (scope) {
          scope.addChangeNotifier<ClockLabViewModel>(() => viewModel);
        },
        child: const MaterialApp(home: ClockLabPage()),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Amostra mais recente'), findsOneWidget);
    expect(find.text('Abertura do laboratório'), findsWidgets);
    expect(find.text('Marcador de início do app'), findsOneWidget);
    expect(find.text('Relógio monotônico (ns)'), findsWidgets);

    await tester.tap(find.text('Ler relógio'));
    await tester.pumpAndSettle();
    expect(find.text('200'), findsWidgets);
    expect(find.text('Amostra manual'), findsWidgets);

    native.clockError = StateError('clock offline');
    await tester.tap(find.text('Ler relógio'));
    await tester.pumpAndSettle();
    expect(
      find.textContaining('Ocorreu um erro inesperado. Tente novamente.'),
      findsOneWidget,
    );
  });
}
