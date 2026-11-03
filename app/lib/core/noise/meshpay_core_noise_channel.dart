import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import 'noise_channel.dart';

class MeshpayCoreNoiseChannel implements NoiseChannel {
  MeshpayCoreNoiseChannel(this._session);

  final NoiseSession _session;

  @override
  bool get isFinished => _session.isFinished();

  @override
  Uint8List get peerPairKey => _session.peerPairKey();

  @override
  Future<Uint8List> writeMessage() =>
      _session.writeMessage(payload: Uint8List(0));

  @override
  Future<Uint8List> readMessage(Uint8List message) =>
      _session.readMessage(message: message);

  @override
  Future<Uint8List> encrypt(Uint8List plaintext) =>
      _session.encrypt(plaintext: plaintext);

  @override
  Future<Uint8List> decrypt(Uint8List ciphertext) =>
      _session.decrypt(ciphertext: ciphertext);
}
