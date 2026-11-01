import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/pages/channel_lab_page.dart';
import 'package:meshpay/features/lab/presentation/view_models/channel_lab_view_model.dart';

import '../../../../fakes/fake_native_lab_service.dart';

void main() {
  testWidgets('runs the echo and displays its results', (tester) async {
    final native = FakeNativeLabService()
      ..loopbackTimingsNanos = List<int>.generate(
        100,
        (index) => (index + 1) * 1000000,
      );
    final viewModel = ChannelLabViewModel(native);
    final module = createModule(register: (_) {});

    await tester.pumpWidget(
      ModularApp(
        module: module,
        provide: (scope) {
          scope.addChangeNotifier<ChannelLabViewModel>(() => viewModel);
        },
        child: const MaterialApp(home: ChannelLabPage()),
      ),
    );

    expect(
      find.text('Eco do canal nativo (25 KB x 100)'),
      findsOneWidget,
    );

    await tester.tap(find.text('Eco do canal nativo (25 KB x 100)'));
    await tester.pumpAndSettle();

    expect(find.text('100 de 100'), findsOneWidget);
    expect(find.text('50.000 ms'), findsOneWidget);
    expect(find.text('95.000 ms'), findsOneWidget);
  });
}
