import 'dart:typed_data';

class HexHelper {
  HexHelper._();

  static String encode(Uint8List bytes) =>
      bytes.map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();

  static bool isValid(String value, int length) =>
      value.length == length &&
      value.codeUnits.every(
        (codeUnit) =>
            (codeUnit >= 48 && codeUnit <= 57) ||
            (codeUnit >= 65 && codeUnit <= 70) ||
            (codeUnit >= 97 && codeUnit <= 102),
      );

  static Uint8List decode(String value) {
    if (value.length.isOdd) {
      throw const FormatException('Hex string must have an even length.');
    }
    return Uint8List.fromList(<int>[
      for (var index = 0; index < value.length; index += 2)
        int.parse(value.substring(index, index + 2), radix: 16),
    ]);
  }
}
