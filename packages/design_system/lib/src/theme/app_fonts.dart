import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

class AppFonts {
  static TextStyle mono({TextStyle? textStyle}) {
    final fontFamily = GoogleFonts.ibmPlexMono(
      fontWeight: textStyle?.fontWeight,
    ).fontFamily;
    if (fontFamily == null) {
      throw StateError('Google Fonts did not provide an IBM Plex Mono family.');
    }

    return (textStyle ?? const TextStyle()).copyWith(fontFamily: fontFamily);
  }
}
