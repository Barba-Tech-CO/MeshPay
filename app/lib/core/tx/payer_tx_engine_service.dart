import 'dart:typed_data';

/// Payer side of the two-phase commit (Rust `PayerTxEngine`).
abstract interface class PayerTxEngineService {
  Future<Uint8List> getPublicKey();

  /// LAB ONLY: `ChequeStub` for [chequeId] issued to this payer's key, sealed
  /// by the lab ISSUER key and expiring `E` after [issuedAtUnix].
  Future<Uint8List> labStub({
    required Uint8List chequeId,
    required int maxAmountUnits,
    required int issuedAtUnix,
  });

  /// Strictly increasing and persisted, so a counter is never reused.
  Future<int> nextCounter();

  /// Opens the engine for one payee; the cheque signature check is real.
  Future<void> open({required Uint8List payeePublicKey});

  /// Returns the TX_PROPOSE bytes, durable before this returns. [stub] is the
  /// serialized server `ChequeStub` for the cheque named in [spendBody].
  Future<Uint8List> propose({
    required Uint8List spendBody,
    required int counter,
    required Uint8List recipientId,
    required Uint8List stub,
  });

  /// Verifies the receiver ack; LOCKED becomes COMMITTED.
  Future<void> onAck(Uint8List ack);

  /// TX_PROPOSE bytes of every cheque still locked, byte for byte as first sent.
  Future<List<Uint8List>> resumePending();

  /// Releases the open engine handle (safe to call when none is open).
  Future<void> close();
}
