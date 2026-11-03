import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/pages/hardware_key_lab_page.dart';
import 'package:meshpay/features/lab/presentation/view_models/hardware_key_lab_view_model.dart';

import '../../../../fakes/fake_hardware_key_lab_service.dart';

void main() {
  late FakeHardwareKeyLabService service;

  Future<void> pumpPage(WidgetTester tester) async {
    service = FakeHardwareKeyLabService();
    final viewModel = HardwareKeyLabViewModel(service);
    await tester.pumpWidget(
      ModularApp(
        module: createModule(register: (_) {}),
        provide: (scope) {
          scope.addChangeNotifier<HardwareKeyLabViewModel>(() => viewModel);
        },
        child: const MaterialApp(home: HardwareKeyLabPage()),
      ),
    );
  }

  testWidgets('shows the probe result after the button', (tester) async {
    await pumpPage(tester);
    expect(find.text('Resultado'), findsNothing);

    await tester.tap(find.text('Gerar e testar chave'));
    await tester.pumpAndSettle();

    expect(find.text('Nível de segurança'), findsOneWidget);
    expect(find.text('TEE'), findsOneWidget);
    expect(find.text('71 bytes DER'), findsOneWidget);
    expect(find.text('3'), findsOneWidget);
    expect(find.text('Apagada'), findsOneWidget);
  });

  testWidgets('StrongBox toggle surfaces strongbox_unavailable', (
    tester,
  ) async {
    await pumpPage(tester);

    await tester.tap(find.text('Exigir StrongBox'));
    await tester.pump();
    await tester.tap(find.text('Gerar e testar chave'));
    await tester.pumpAndSettle();

    expect(find.textContaining('não tem StrongBox'), findsOneWidget);
    expect(find.text('Resultado'), findsNothing);
  });
}
