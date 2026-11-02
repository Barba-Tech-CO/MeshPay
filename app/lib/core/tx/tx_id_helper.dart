import 'dart:typed_data';

import '../noise/hex_helper.dart';

class TxIdHelper {
  TxIdHelper._();

  static const int _shortHexChars = 8;

  static String? shortHex(Uint8List? txId) =>
      txId == null ? null : HexHelper.encode(txId).substring(0, _shortHexChars);
}
