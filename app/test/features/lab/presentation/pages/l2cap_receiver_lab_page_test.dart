import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/pages/l2cap_receiver_lab_page.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_receiver_lab_view_model.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_noise_service.dart';
import '../../../../fakes/fake_permission_service.dart';

void main() {
  testWidgets('shows receiver controls and current closed state', (
    tester,
  ) async {
    final viewModel = L2capReceiverLabViewModel(
      FakeNativeLabService(),
      FakePermissionService(),
      FakeNoiseService(),
    );
    final module = createModule(register: (_) {});

    await tester.pumpWidget(
      ModularApp(
        module: module,
        provide: (scope) {
          scope.addChangeNotifier<L2capReceiverLabViewModel>(() => viewModel);
        },
        child: const MaterialApp(home: L2capReceiverLabPage()),
      ),
    );

    expect(find.text('Receptor L2CAP'), findsOneWidget);
    expect(find.text('Iniciar recepção'), findsOneWidget);
    expect(find.text('Mensagens recebidas'), findsOneWidget);
    expect(find.text('Fechado'), findsOneWidget);
    await tester.scrollUntilVisible(
      find.text('Mensagens (bytes)'),
      240,
      scrollable: find
          .descendant(
            of: find.byType(ListView),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(find.text('Handshake Noise XK'), findsOneWidget);
  });
}
