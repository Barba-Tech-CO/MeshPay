import 'dart:convert';

import '../../../../core/noise/hex_helper.dart';

typedef L2capPayerInput = ({String sessionIdHex, int psm, String keyHex});

class L2capPayerInputHelper {
  L2capPayerInputHelper._();

  static const int _sessionIdHexLength = 16;
  static const int _keyHexLength = 64;
  static const int _maxPsm = 65535;

  static bool isValidSessionId(String value) =>
      HexHelper.isValid(value, _sessionIdHexLength);

  static L2capPayerInput parseQr(String payload) {
    final decoded = jsonDecode(payload);
    if (decoded is! Map<String, Object?>) {
      throw const FormatException('O conteúdo do QR não é um objeto.');
    }
    final session = decoded['session'];
    final psm = decoded['psm'];
    final key = decoded['key'];
    if (session is! String || !isValidSessionId(session)) {
      throw const FormatException('O ID de sessão do QR é inválido.');
    }
    if (psm is! int || psm < 1 || psm > _maxPsm) {
      throw const FormatException('O PSM do QR é inválido.');
    }
    if (key is! String || !HexHelper.isValid(key, _keyHexLength)) {
      throw const FormatException('A chave do QR é inválida.');
    }
    return (sessionIdHex: session, psm: psm, keyHex: key);
  }

  /// Validates manual entry; the message of the thrown [FormatException] is
  /// shown to the user. Hex values come back lower-cased.
  static L2capPayerInput parseManual({
    required String sessionIdHex,
    required String psm,
    required String keyHex,
  }) {
    final session = sessionIdHex.trim().toLowerCase();
    if (!isValidSessionId(session)) {
      throw const FormatException(
        'Informe um ID de sessão válido em hexadecimal (8 bytes).',
      );
    }
    final parsedPsm = int.tryParse(psm.trim());
    if (parsedPsm == null || parsedPsm < 1 || parsedPsm > _maxPsm) {
      throw const FormatException('Informe um PSM válido entre 1 e 65535.');
    }
    final key = keyHex.trim().toLowerCase();
    if (!HexHelper.isValid(key, _keyHexLength)) {
      throw const FormatException(
        'Informe a chave pública do receptor em hexadecimal (32 bytes, '
        '64 caracteres).',
      );
    }
    return (sessionIdHex: session, psm: parsedPsm, keyHex: key);
  }
}
