import '../../../../core/server/client/server_error_code.dart';
import '../../../../core/server/client/server_exception.dart';
import '../../../../core/server/messages/redeem_status.dart';
import 'payment_key_message_helper.dart';

/// User-facing text for server failures; never shows peer-provided text.
class ServerLabMessageHelper {
  ServerLabMessageHelper._();

  static const String registerFirst = 'Registre a conta primeiro.';
  static const String issueFirst =
      'Emita um talão com cheques disponíveis primeiro.';

  static const String snapshotUnavailable =
      'Snapshot de revogação indisponível no servidor.';

  static String unixToText(int seconds) => DateTime.fromMillisecondsSinceEpoch(
    seconds * 1000,
    isUtc: true,
  ).toIso8601String();

  static String messageFor(Object failure) {
    if (failure is StateError) {
      return failure.message;
    }
    if (failure is! ServerException) {
      return PaymentKeyMessageHelper.messageFor(failure);
    }
    final text = switch (failure.code) {
      ServerErrorCode.badRequest =>
        'O servidor recusou a requisição malformada.',
      ServerErrorCode.forbidden =>
        'Recusado pelo servidor (assinatura, atestação ou conta).',
      ServerErrorCode.notFound => 'Conta não encontrada no servidor.',
      ServerErrorCode.unprocessable =>
        'O servidor recusou os valores (limites do talão ou do ledger).',
      ServerErrorCode.tooManyRequests =>
        'Limite de requisições atingido. Aguarde um minuto.',
      ServerErrorCode.internal => 'Erro interno do servidor.',
      ServerErrorCode.unavailable =>
        'Servidor ocupado ou sem resposta a tempo.',
      ServerErrorCode.unexpectedStatus => 'Resposta inesperada do servidor.',
      ServerErrorCode.unreachable =>
        'Servidor inacessível. Confira a URL e o adb reverse.',
      ServerErrorCode.malformedResponse =>
        'A resposta do servidor não pôde ser lida.',
      ServerErrorCode.insecureUrl =>
        'URL insegura: use https (http só para 127.0.0.1 ou localhost).',
    };
    final status = failure.httpStatus;
    return status == null
        ? '$text [${failure.code.name}]'
        : '$text [${failure.code.name}, HTTP $status]';
  }

  static String yesNo(bool value) => value ? 'Sim' : 'Não';

  static String redeemStatusLabel(RedeemStatus status) => switch (status) {
    RedeemStatus.paid => 'Pago',
    RedeemStatus.alreadyRedeemedSameBody => 'Já pago (mesmo corpo)',
    RedeemStatus.rejectedConflict => 'Recusado: conflito',
    RedeemStatus.rejectedExpired => 'Recusado: expirado',
    RedeemStatus.rejectedRevoked => 'Recusado: revogado',
    RedeemStatus.rejectedInvalid => 'Recusado: inválido',
    RedeemStatus.unspecified => 'Não informado',
  };
}
