import 'dart:typed_data';

abstract interface class NoiseChannel {
  bool get isFinished;

  Future<Uint8List> writeMessage();

  Future<Uint8List> readMessage(Uint8List message);

  Future<Uint8List> encrypt(Uint8List plaintext);

  Future<Uint8List> decrypt(Uint8List ciphertext);
}
