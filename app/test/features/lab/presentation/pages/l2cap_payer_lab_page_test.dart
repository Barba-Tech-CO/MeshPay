import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/pages/l2cap_payer_lab_page.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_payer_lab_view_model.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_permission_service.dart';

void main() {
  testWidgets('shows QR, manual connection fields, and echo controls', (
    tester,
  ) async {
    final viewModel = L2capPayerLabViewModel(
      FakeNativeLabService(),
      FakePermissionService(),
    );
    final module = createModule(register: (_) {});

    await tester.pumpWidget(
      ModularApp(
        module: module,
        provide: (scope) {
          scope.addChangeNotifier<L2capPayerLabViewModel>(() => viewModel);
        },
        child: const MaterialApp(home: L2capPayerLabPage()),
      ),
    );

    expect(find.text('Pagador L2CAP'), findsOneWidget);
    expect(find.text('Ler QR do receptor'), findsOneWidget);
    expect(find.text('ID da sessão (hex)'), findsOneWidget);
    expect(find.text('PSM'), findsOneWidget);
    expect(find.text('ID do dispositivo'), findsOneWidget);

    await tester.scrollUntilVisible(
      find.text('Executar eco'),
      240,
      scrollable: find
          .descendant(
            of: find.byType(ListView),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.pumpAndSettle();
    expect(find.text('Executar eco'), findsOneWidget);
  });
}
