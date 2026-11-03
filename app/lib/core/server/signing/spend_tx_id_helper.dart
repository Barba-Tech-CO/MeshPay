import 'dart:convert';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';

import 'server_signing_message_helper.dart';

/// tx_id = SHA-256("meshpay/tx_id/v1" || counter (u64 BE) ||
/// SHA-256(spend body without tx_id)); server/src/crypto/spend_hashing.rs.
class SpendTxIdHelper {
  SpendTxIdHelper._();

  static Uint8List derive(int counter, Uint8List bodyWithoutTxId) =>
      Uint8List.fromList(
        sha256.convert(<int>[
          ...utf8.encode('meshpay/tx_id/v1'),
          ...ServerSigningMessageHelper.bigEndianUint64(counter),
          ...sha256.convert(bodyWithoutTxId).bytes,
        ]).bytes,
      );
}
