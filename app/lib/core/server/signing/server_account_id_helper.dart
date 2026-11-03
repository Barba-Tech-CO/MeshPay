import 'dart:typed_data';

import 'package:crypto/crypto.dart';

/// Account id = first 16 bytes of SHA-256 of the compressed public key
/// (`key_id` in server/src/crypto/p256_keys.rs).
class ServerAccountIdHelper {
  ServerAccountIdHelper._();

  static const int _accountIdBytes = 16;

  static Uint8List derive(Uint8List compressedPublicKey) => Uint8List.fromList(
    sha256.convert(compressedPublicKey).bytes.sublist(0, _accountIdBytes),
  );
}
