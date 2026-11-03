import 'dart:typed_data';

import '../../tx/lab_public_key_helper.dart';

/// The server stores compressed SEC1 keys (33 bytes, `accounts` CHECK); the
/// Keystore returns the uncompressed point (65 bytes).
class Sec1KeyHelper {
  Sec1KeyHelper._();

  static Uint8List compress(Uint8List publicKey) {
    if (!LabPublicKeyHelper.isValid(publicKey)) {
      throw const FormatException('Invalid SEC1 P-256 public key.');
    }
    if (publicKey.length == LabPublicKeyHelper.compressedBytes) {
      return publicKey;
    }
    final prefix = publicKey.last.isEven ? 2 : 3;
    return Uint8List.fromList(<int>[prefix, ...publicKey.sublist(1, 33)]);
  }
}
