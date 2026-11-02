import 'dart:typed_data';

import 'package:meshpay/core/noise/noise_channel.dart';

/// Deterministic stand-in for the Rust Noise XK session: same message sizes
/// (48/48/64), and the handshake only succeeds when both sides agree on the
/// responder public key and the prologue.
class FakeNoiseChannel implements NoiseChannel {
  FakeNoiseChannel({required this.isInitiator, required this.binding});

  static const int tagBytes = 16;

  final bool isInitiator;
  final int binding;
  int _step = 0;

  @override
  bool get isFinished => _step == 3;

  Uint8List _message(int length) {
    final message = Uint8List(length);
    ByteData.sublistView(message).setUint32(0, binding & 0xffffffff);
    return message;
  }

  void _expect(Uint8List message, int length) {
    if (message.length != length ||
        ByteData.sublistView(message).getUint32(0) != binding & 0xffffffff) {
      throw StateError('Noise handshake failed.');
    }
  }

  @override
  Future<Uint8List> writeMessage() async {
    if (isInitiator && _step == 0) {
      _step = 1;
      return _message(48);
    }
    if (isInitiator && _step == 2) {
      _step = 3;
      return _message(64);
    }
    if (!isInitiator && _step == 1) {
      _step = 2;
      return _message(48);
    }
    throw StateError('Unexpected handshake write.');
  }

  @override
  Future<Uint8List> readMessage(Uint8List message) async {
    if (isInitiator && _step == 1) {
      _expect(message, 48);
      _step = 2;
    } else if (!isInitiator && _step == 0) {
      _expect(message, 48);
      _step = 1;
    } else if (!isInitiator && _step == 2) {
      _expect(message, 64);
      _step = 3;
    } else {
      throw StateError('Unexpected handshake read.');
    }
    return Uint8List(0);
  }

  @override
  Future<Uint8List> encrypt(Uint8List plaintext) async {
    if (!isFinished) {
      throw StateError('Handshake not finished.');
    }
    return Uint8List.fromList(<int>[
      ...plaintext.map((byte) => byte ^ 0x5a),
      ...List<int>.filled(tagBytes, 0xa5),
    ]);
  }

  @override
  Future<Uint8List> decrypt(Uint8List ciphertext) async {
    if (!isFinished) {
      throw StateError('Handshake not finished.');
    }
    if (ciphertext.length < tagBytes ||
        ciphertext
            .sublist(ciphertext.length - tagBytes)
            .any((byte) => byte != 0xa5)) {
      throw StateError('Authentication failed.');
    }
    return Uint8List.fromList(
      ciphertext
          .sublist(0, ciphertext.length - tagBytes)
          .map((byte) => byte ^ 0x5a)
          .toList(),
    );
  }
}
