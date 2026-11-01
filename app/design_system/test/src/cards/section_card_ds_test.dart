import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('shows its title and child without owning copy', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SectionCardDs(title: 'Seção', child: Text('Conteúdo')),
        ),
      ),
    );

    expect(find.text('Seção'), findsOneWidget);
    expect(find.text('Conteúdo'), findsOneWidget);
  });
}
