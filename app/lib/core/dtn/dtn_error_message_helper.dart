/// Portuguese text for the engine's rejection outcomes. The engine reports a
/// rejection as `envelope rejected: <Outcome>` (the Debug name of the Rust
/// `IngestOutcome` variant); any other engine text is not a rejection.
class DtnErrorMessageHelper {
  DtnErrorMessageHelper._();

  static const String rejectionPrefix = 'envelope rejected: ';

  static const Map<String, String> _messages = <String, String>{
    'SessionBudgetExceeded':
        'O limite de 20 envelopes desta conexão foi atingido.',
    'Refused': 'O buffer está cheio e não aceitou o envelope.',
    'RateLimited': 'O limite global de envelopes por minuto foi atingido.',
    'Oversize': 'O envelope excede o tamanho máximo.',
    'Malformed': 'O envelope recebido está malformado.',
    'IdMismatch': 'O identificador do envelope não confere com o conteúdo.',
    'HopsExceeded': 'O envelope esgotou os saltos permitidos.',
    'Expired': 'O envelope recebido já expirou.',
    'FromFuture': 'O envelope recebido tem data no futuro.',
    'UnverifiedProofs':
        'As provas do envelope ainda não podem ser verificadas.',
  };

  /// The outcome name when [engineMessage] contains a rejection, else null.
  /// The prefix may sit anywhere (flutter_rust_bridge wraps the text); the
  /// outcome is the run of ASCII letters right after it.
  static String? outcomeOf(String engineMessage) {
    final start = engineMessage.indexOf(rejectionPrefix);
    if (start < 0) {
      return null;
    }
    final from = start + rejectionPrefix.length;
    var end = from;
    while (end < engineMessage.length) {
      final unit = engineMessage.codeUnitAt(end);
      final isUpper = unit >= 0x41 && unit <= 0x5A;
      final isLower = unit >= 0x61 && unit <= 0x7A;
      if (!isUpper && !isLower) {
        break;
      }
      end++;
    }
    if (end == from) {
      return null;
    }
    return engineMessage.substring(from, end);
  }

  static String messageFor(String outcome) =>
      _messages[outcome] ?? 'O buffer recusou o envelope.';
}
