import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/tx_error_code.dart';
import 'package:meshpay/core/tx/tx_error_message_helper.dart';
import 'package:meshpay/core/tx/tx_lab_exception.dart';

void main() {
  test('maps engine errors to fixed codes and never echoes raw text', () {
    final failure = Exception('tx_id does not match counter and content');

    expect(TxErrorMessageHelper.codeFor(failure), TxErrorCode.txIdMismatch);
    expect(
      TxErrorMessageHelper.messageFor(failure),
      'O identificador do cheque não confere com o conteúdo.',
    );
    final unknown = Exception('secret path /data/db boom');
    expect(TxErrorMessageHelper.codeFor(unknown), TxErrorCode.internal);
    expect(
      TxErrorMessageHelper.messageFor(unknown),
      'Falha interna na transação.',
    );
  });

  test('prefixes a remote refusal and survives an unknown wire code', () {
    expect(
      TxErrorMessageHelper.messageFor(
        const TxLabException(TxErrorCode.badSignature, isRemote: true),
      ),
      'Receptor recusou: A assinatura do cheque ou do recibo não confere.',
    );
    expect(TxErrorCode.fromWire(200), TxErrorCode.internal);
    expect(
      TxErrorCode.fromWire(TxErrorCode.tooLarge.wire),
      TxErrorCode.tooLarge,
    );
  });
}
