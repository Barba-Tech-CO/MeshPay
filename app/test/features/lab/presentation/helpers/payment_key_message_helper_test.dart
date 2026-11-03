import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/helpers/payment_key_message_helper.dart';
import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/payment_key_exception.dart';

void main() {
  test('maps every documented Keystore code to Portuguese', () {
    const codes = <String>[
      'alias_exists',
      'invalid_alias',
      'invalid_count',
      'data_too_large',
      'attestation_unavailable',
      'strongbox_unavailable',
      'max_usage_count_unsupported',
      'key_not_found',
      'key_unusable',
      'sign_failed',
      'keystore_error',
      'plugin_detached',
    ];

    for (final code in codes) {
      expect(
        PaymentKeyMessageHelper.messageFor(PaymentKeyException(code, null)),
        isNot(contains('Falha na chave de hardware')),
        reason: code,
      );
    }
  });

  test('unknown codes keep the code and other errors use the lab message', () {
    expect(
      PaymentKeyMessageHelper.messageFor(
        const PaymentKeyException('novo_codigo', null),
      ),
      contains('novo_codigo'),
    );
    expect(
      PaymentKeyMessageHelper.messageFor(StateError('x')),
      'Ocorreu um erro inesperado. Tente novamente.',
    );
  });

  test('labels each security level', () {
    expect(
      KeySecurityLevel.values.map(PaymentKeyMessageHelper.levelLabel),
      <String>['Software', 'TEE', 'StrongBox', 'Desconhecido'],
    );
  });
}
