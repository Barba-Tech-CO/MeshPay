import 'dart:typed_data';

import 'package:meshpay/core/noise/noise_channel.dart';
import 'package:meshpay/core/noise/noise_service.dart';

import 'fake_noise_channel.dart';

class FakeNoiseService implements NoiseService {
  final List<FakeNoiseChannel> initiators = <FakeNoiseChannel>[];
  final List<FakeNoiseChannel> responders = <FakeNoiseChannel>[];
  final List<Uint8List> responderPrivateKeys = <Uint8List>[];
  final List<Uint8List> responderPrologues = <Uint8List>[];
  final List<Uint8List> initiatorPrologues = <Uint8List>[];
  int keypairCount = 0;

  static Uint8List _publicFor(Uint8List privateKey) =>
      Uint8List.fromList(privateKey.map((byte) => byte ^ 0xff).toList());

  @override
  ({Uint8List publicKey, Uint8List privateKey}) generateStaticKeypair() {
    keypairCount++;
    final privateKey = Uint8List.fromList(
      List<int>.filled(32, keypairCount),
    );
    return (publicKey: _publicFor(privateKey), privateKey: privateKey);
  }

  static int _binding(Uint8List publicKey, Uint8List prologue) =>
      Object.hashAll(<Object>[...publicKey, ...prologue]);

  @override
  NoiseChannel newInitiator({
    required Uint8List responderPublicKey,
    required Uint8List prologue,
  }) {
    initiatorPrologues.add(prologue);
    final channel = FakeNoiseChannel(
      isInitiator: true,
      binding: _binding(responderPublicKey, prologue),
    );
    initiators.add(channel);
    return channel;
  }

  @override
  NoiseChannel newResponder({
    required Uint8List localPrivateKey,
    required Uint8List prologue,
  }) {
    responderPrivateKeys.add(localPrivateKey);
    responderPrologues.add(prologue);
    final channel = FakeNoiseChannel(
      isInitiator: false,
      binding: _binding(_publicFor(localPrivateKey), prologue),
    );
    responders.add(channel);
    return channel;
  }
}
