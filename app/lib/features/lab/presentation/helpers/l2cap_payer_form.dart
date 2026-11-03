import 'l2cap_payer_input_helper.dart';

/// The payer page's connection fields (QR or manual entry).
class L2capPayerForm {
  String sessionIdHex = '';
  String psm = '';
  String keyHex = '';
  String deviceId = '';

  /// Fills the fields from a scanned QR; returns the error text, or null.
  String? applyQr(String payload) {
    try {
      final input = L2capPayerInputHelper.parseQr(payload);
      sessionIdHex = input.sessionIdHex;
      psm = '${input.psm}';
      keyHex = input.keyHex;
      return null;
    } on FormatException catch (failure) {
      return 'QR inválido: ${failure.message}';
    }
  }

  /// Throws [FormatException] (message shown to the user) on invalid fields.
  ({L2capPayerInput input, String deviceId}) parseForConnect() {
    final input = L2capPayerInputHelper.parseManual(
      sessionIdHex: sessionIdHex,
      psm: psm,
      keyHex: keyHex,
    );
    final target = deviceId.trim();
    if (target.isEmpty) {
      throw const FormatException(
        'Informe o ID do dispositivo encontrado pelo scan.',
      );
    }
    return (input: input, deviceId: target);
  }
}
