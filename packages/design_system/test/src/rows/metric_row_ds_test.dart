import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:google_fonts/google_fonts.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() {
    GoogleFonts.config.allowRuntimeFetching = false;
  });

  testWidgets('shows values and applies mono typography when requested', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: MetricRowDs(label: 'Sequência', value: '12', mono: true),
        ),
      ),
    );

    expect(find.text('Sequência'), findsOneWidget);
    final value = tester.widget<Text>(find.text('12'));
    expect(value.style?.fontFamily, contains('IBMPlexMono'));
  });
}
