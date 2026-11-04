import 'dart:typed_data';

abstract interface class NoiseChannel {
  bool get isFinished;

  /// Initiator static key || responder static key; only after the handshake.
  Uint8List get peerPairKey;

  Future<Uint8List> writeMessage();

  Future<Uint8List> readMessage(Uint8List message);

  Future<Uint8List> encrypt(Uint8List plaintext);

  Future<Uint8List> decrypt(Uint8List ciphertext);
}
