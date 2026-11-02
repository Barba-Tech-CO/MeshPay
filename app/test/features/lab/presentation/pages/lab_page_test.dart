import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:meshpay/features/lab/presentation/pages/lab_page.dart';

void main() {
  testWidgets('shows the laboratory destinations', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: LabPage()));

    expect(find.text('Laboratório'), findsOneWidget);
    expect(find.text('Relógio e reinício'), findsOneWidget);
    expect(find.text('Canal nativo'), findsOneWidget);
    expect(find.text('Bluetooth'), findsOneWidget);
    expect(find.text('Canal L2CAP (Fase 1)'), findsOneWidget);
    expect(find.text('Receptor L2CAP'), findsOneWidget);
    expect(find.text('Pagador L2CAP'), findsOneWidget);
  });
}
