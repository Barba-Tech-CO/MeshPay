import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

import 'app_colors.dart';

class AppTheme {
  static final ThemeData light = _buildTheme(isDark: false);
  static final ThemeData dark = _buildTheme(isDark: true);

  static const TextTheme _textThemeSizes = TextTheme(
    displayLarge: TextStyle(fontSize: 32),
    displayMedium: TextStyle(fontSize: 28),
    displaySmall: TextStyle(fontSize: 24),
    headlineLarge: TextStyle(fontSize: 24),
    headlineMedium: TextStyle(fontSize: 22),
    headlineSmall: TextStyle(fontSize: 20),
    titleLarge: TextStyle(
      fontSize: 20,
      fontWeight: FontWeight.w600,
    ),
    titleMedium: TextStyle(fontSize: 16),
    titleSmall: TextStyle(fontSize: 14),
    bodyLarge: TextStyle(fontSize: 16),
    bodyMedium: TextStyle(fontSize: 14),
    bodySmall: TextStyle(fontSize: 12),
    labelLarge: TextStyle(fontSize: 14),
    labelMedium: TextStyle(fontSize: 12),
    labelSmall: TextStyle(fontSize: 10),
  );

  static final TextTheme _textTheme = TextTheme(
    displayLarge: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.displayLarge,
    ),
    displayMedium: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.displayMedium,
    ),
    displaySmall: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.displaySmall,
    ),
    headlineLarge: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.headlineLarge,
    ),
    headlineMedium: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.headlineMedium,
    ),
    headlineSmall: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.headlineSmall,
    ),
    titleLarge: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.titleLarge,
    ),
    titleMedium: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.titleMedium,
    ),
    titleSmall: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.titleSmall,
    ),
    bodyLarge: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.bodyLarge,
    ),
    bodyMedium: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.bodyMedium,
    ),
    bodySmall: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.bodySmall,
    ),
    labelLarge: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.labelLarge,
    ),
    labelMedium: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.labelMedium,
    ),
    labelSmall: GoogleFonts.ibmPlexSans(
      textStyle: _textThemeSizes.labelSmall,
    ),
  );

  static ThemeData _buildTheme({required bool isDark}) {
    final brightness = isDark ? Brightness.dark : Brightness.light;
    final backgroundColor = isDark
        ? AppColors.backgroundColorDarkTheme
        : AppColors.backgroundColor;
    final surfaceColor = isDark
        ? AppColors.surfaceColorDarkTheme
        : AppColors.surfaceColor;
    final borderColor = isDark
        ? AppColors.borderColorDarkTheme
        : AppColors.borderColor;
    final primaryTextColor = isDark
        ? AppColors.primaryTextColorDarkTheme
        : AppColors.primaryTextColor;
    final accentColor = isDark
        ? AppColors.accentColorDarkTheme
        : AppColors.accentColor;
    final onAccentColor = isDark
        ? AppColors.onAccentColorDarkTheme
        : AppColors.onAccentColor;
    final disabledColor = isDark
        ? AppColors.disabledColorDarkTheme
        : AppColors.disabledColor;
    final errorColor = isDark
        ? AppColors.errorColorDarkTheme
        : AppColors.errorColor;
    final colorScheme = isDark
        ? ColorScheme.dark(
            primary: accentColor,
            onPrimary: onAccentColor,
            secondary: accentColor,
            onSecondary: onAccentColor,
            error: errorColor,
            onError: onAccentColor,
            surface: surfaceColor,
            onSurface: primaryTextColor,
            outline: AppColors.inputBorderColorDarkTheme,
            outlineVariant: borderColor,
            tertiary: AppColors.successColorDarkTheme,
            onTertiary: onAccentColor,
          )
        : ColorScheme.light(
            primary: accentColor,
            onPrimary: onAccentColor,
            secondary: accentColor,
            onSecondary: onAccentColor,
            error: errorColor,
            onError: onAccentColor,
            surface: surfaceColor,
            onSurface: primaryTextColor,
            outline: AppColors.inputBorderColor,
            outlineVariant: borderColor,
            tertiary: AppColors.successColor,
            onTertiary: onAccentColor,
          );

    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: colorScheme,
      scaffoldBackgroundColor: backgroundColor,
      textTheme: _textTheme.apply(
        bodyColor: primaryTextColor,
        displayColor: primaryTextColor,
      ),
      inputDecorationTheme: InputDecorationTheme(
        enabledBorder: _inputBorder(AppColors.inputBorderColor),
        focusedBorder: _inputBorder(accentColor, width: 2),
        errorBorder: _inputBorder(errorColor, width: 2),
        focusedErrorBorder: _inputBorder(errorColor, width: 2),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: ButtonStyle(
          backgroundColor: _buttonColor(accentColor, disabledColor),
          foregroundColor: _buttonColor(onAccentColor, disabledColor),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: ButtonStyle(
          foregroundColor: _buttonColor(accentColor, disabledColor),
          side: WidgetStateProperty.resolveWith<BorderSide>(
            (states) => BorderSide(
              color: states.contains(WidgetState.disabled)
                  ? disabledColor
                  : accentColor,
              width: 2,
            ),
          ),
        ),
      ),
    );
  }

  static OutlineInputBorder _inputBorder(Color color, {double width = 1}) {
    return OutlineInputBorder(
      borderRadius: BorderRadius.circular(4),
      borderSide: BorderSide(
        color: color,
        width: width,
      ),
    );
  }

  static WidgetStateProperty<Color> _buttonColor(
    Color enabledColor,
    Color disabledColor,
  ) {
    return WidgetStateProperty.resolveWith(
      (states) {
        return states.contains(WidgetState.disabled)
            ? disabledColor
            : enabledColor;
      },
    );
  }
}
