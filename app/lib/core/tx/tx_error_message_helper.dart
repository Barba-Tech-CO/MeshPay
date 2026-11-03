import 'tx_error_code.dart';
import 'tx_lab_exception.dart';

/// Maps engine errors to fixed [TxErrorCode]s and codes to Portuguese text.
class TxErrorMessageHelper {
  TxErrorMessageHelper._();

  static const List<(String, TxErrorCode)> _fragments = <(String, TxErrorCode)>[
    ('not addressed to this account', TxErrorCode.notAddressedToUs),
    ('signature', TxErrorCode.badSignature),
    ('tx_id does not match', TxErrorCode.txIdMismatch),
    ('with different content', TxErrorCode.conflict),
    ('exceeds the size cap', TxErrorCode.tooLarge),
    ('engine not open', TxErrorCode.helloMissing),
    ('cannot commit', TxErrorCode.notCommittable),
  ];

  static TxErrorCode codeFor(Object failure) {
    if (failure is TxLabException) {
      return failure.code;
    }
    if (failure is FormatException) {
      return TxErrorCode.invalidMessage;
    }
    final text = '$failure';
    for (final (fragment, code) in _fragments) {
      if (text.contains(fragment)) {
        return code;
      }
    }
    return TxErrorCode.internal;
  }

  static String messageForCode(TxErrorCode code) => switch (code) {
    TxErrorCode.invalidMessage => 'Mensagem de transação inválida.',
    TxErrorCode.invalidKey => 'Chave pública inválida na troca de chaves.',
    TxErrorCode.helloRepeated => 'A troca de chaves já foi feita nesta sessão.',
    TxErrorCode.helloMissing => 'A troca de chaves ainda não foi feita.',
    TxErrorCode.notAddressedToUs => 'O cheque não é para esta conta.',
    TxErrorCode.badSignature =>
      'A assinatura do cheque ou do recibo não confere.',
    TxErrorCode.txIdMismatch =>
      'O identificador do cheque não confere com o conteúdo.',
    TxErrorCode.conflict =>
      'Este cheque ou contador já foi usado com outro conteúdo.',
    TxErrorCode.tooLarge => 'A proposta excede o tamanho máximo.',
    TxErrorCode.notCommittable => 'O cheque não pode mais ser confirmado.',
    TxErrorCode.internal => 'Falha interna na transação.',
    TxErrorCode.dtnSummaryRepeated =>
      'O resumo DTN já foi trocado nesta conexão.',
    TxErrorCode.dtnRoundLimit =>
      'A troca DTN excedeu o limite de rodadas desta conexão.',
  };

  static String messageFor(Object failure) {
    final message = messageForCode(codeFor(failure));
    return failure is TxLabException && failure.isRemote
        ? 'Receptor recusou: $message'
        : message;
  }
}
