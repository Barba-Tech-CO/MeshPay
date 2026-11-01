import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:google_fonts/google_fonts.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() {
    GoogleFonts.config.allowRuntimeFetching = false;
  });

  group('AppTheme font families', () {
    test('light and dark themes use IBM Plex Sans', () {
      expect(
        AppTheme.light.textTheme.bodyLarge?.fontFamily,
        contains('IBMPlexSans'),
      );
      expect(
        AppTheme.dark.textTheme.bodyLarge?.fontFamily,
        contains('IBMPlexSans'),
      );
    });

    test(
      'light and dark title styles use the IBM Plex Sans semibold family',
      () {
        final semiboldFamily = GoogleFonts.ibmPlexSans(
          fontWeight: FontWeight.w600,
        ).fontFamily;

        expect(semiboldFamily, 'IBMPlexSans_600');

        for (final theme in [AppTheme.light, AppTheme.dark]) {
          expect(
            theme.textTheme.titleLarge!
                .copyWith(fontWeight: FontWeight.w600)
                .fontFamily,
            semiboldFamily,
          );
        }
      },
    );

    test('mono style uses IBM Plex Mono', () {
      expect(AppFonts.mono().fontFamily, contains('IBMPlexMono'));
    });
  });
}
