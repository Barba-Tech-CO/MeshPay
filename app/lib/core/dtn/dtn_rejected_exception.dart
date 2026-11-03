/// An envelope the buffer refused; [reason] is the engine's outcome name
/// (for example `SessionBudgetExceeded`).
class DtnRejectedException implements Exception {
  const DtnRejectedException(this.reason);

  final String reason;
}
