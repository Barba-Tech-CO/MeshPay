import 'dart:typed_data';

/// Lab HELLO keys: SEC1 P-256 point, compressed (33 bytes, software engines)
/// or uncompressed (65 bytes, Keystore).
class LabPublicKeyHelper {
  LabPublicKeyHelper._();

  static const int compressedBytes = 33;
  static const int uncompressedBytes = 65;

  static bool isValid(Uint8List key) => switch (key.length) {
    compressedBytes => key[0] == 2 || key[0] == 3,
    uncompressedBytes => key[0] == 4,
    _ => false,
  };
}
