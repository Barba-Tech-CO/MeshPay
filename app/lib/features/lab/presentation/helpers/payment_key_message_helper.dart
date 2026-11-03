import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/payment_key_exception.dart';

import 'lab_error_message_helper.dart';

class PaymentKeyMessageHelper {
  PaymentKeyMessageHelper._();

  static const Map<String, String> _messages = <String, String>{
    'alias_exists': 'Já existe uma chave com este nome. Apague a chave antiga.',
    'invalid_alias': 'O nome da chave é inválido.',
    'invalid_count': 'A quantidade de chaves é inválida.',
    'data_too_large': 'Os dados a assinar excedem o limite de 64 KiB.',
    'attestation_unavailable':
        'Este aparelho não gerou a cadeia de atestação da chave.',
    'strongbox_unavailable':
        'Este aparelho não tem StrongBox. Desligue "Exigir StrongBox".',
    'max_usage_count_unsupported':
        'Chave de uso único exige Android 12 (API 31) ou mais novo.',
    'key_not_found': 'A chave não foi encontrada no Keystore.',
    'key_unusable': 'A chave existe, mas não pode ser usada agora.',
    'sign_failed': 'O Keystore não conseguiu assinar os dados.',
    'keystore_error': 'Falha no Keystore do Android.',
    'plugin_detached': 'O plugin nativo não está ativo.',
  };

  static String messageFor(Object failure) {
    if (failure is! PaymentKeyException) {
      return LabErrorMessageHelper.messageFor(failure);
    }
    return _messages[failure.code] ??
        'Falha na chave de hardware: ${failure.code}';
  }

  static String levelLabel(KeySecurityLevel level) => switch (level) {
    KeySecurityLevel.software => 'Software',
    KeySecurityLevel.tee => 'TEE',
    KeySecurityLevel.strongBox => 'StrongBox',
    KeySecurityLevel.unknown => 'Desconhecido',
  };
}
