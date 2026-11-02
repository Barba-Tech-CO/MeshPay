import 'dart:convert';
import 'dart:typed_data';

class NoisePrologueHelper {
  NoisePrologueHelper._();

  static Uint8List build({
    required String sessionIdHex,
    required int psm,
    required String keyHex,
  }) => Uint8List.fromList(
    utf8.encode('meshpay/1|$sessionIdHex|$psm|$keyHex'),
  );
}
