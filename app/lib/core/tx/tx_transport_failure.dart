/// The encrypted exchange itself failed (link dropped, timeout), as opposed to
/// an engine or receiver refusal.
class TxTransportFailure implements Exception {
  const TxTransportFailure(this.cause);

  final Object cause;
}
