import 'dart:typed_data';

import 'noise_channel.dart';

abstract interface class NoiseService {
  ({Uint8List publicKey, Uint8List privateKey}) generateStaticKeypair();

  NoiseChannel newInitiator({
    required Uint8List responderPublicKey,
    required Uint8List prologue,
  });

  NoiseChannel newResponder({
    required Uint8List localPrivateKey,
    required Uint8List prologue,
  });
}
