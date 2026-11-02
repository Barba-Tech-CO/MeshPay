import 'dart:typed_data';

/// Receiver side of the two-phase commit (Rust `ReceiverTxEngine`).
abstract interface class ReceiverTxEngineService {
  Future<Uint8List> getPublicKey();

  /// Opens the engine for one payer; the account id is the own public key.
  Future<void> open({required Uint8List payerPublicKey});

  /// Verifies and stores the cheque, returns the signed ack bytes.
  Future<Uint8List> onPropose(Uint8List propose);

  /// Releases the open engine handle (safe to call when none is open).
  Future<void> close();
}
