import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import 'meshpay_core_noise_channel.dart';
import 'noise_channel.dart';
import 'noise_service.dart';

class MeshpayCoreNoiseService implements NoiseService {
  @override
  ({Uint8List publicKey, Uint8List privateKey}) generateStaticKeypair() {
    final keypair = generateNoiseStaticKeypair();
    return (publicKey: keypair.publicKey, privateKey: keypair.privateKey);
  }

  @override
  NoiseChannel newInitiator({
    required Uint8List responderPublicKey,
    required Uint8List prologue,
  }) => MeshpayCoreNoiseChannel(
    NoiseSession.newInitiator(
      responderPublicKey: responderPublicKey,
      prologue: prologue,
    ),
  );

  @override
  NoiseChannel newResponder({
    required Uint8List localPrivateKey,
    required Uint8List prologue,
  }) => MeshpayCoreNoiseChannel(
    NoiseSession.newResponder(
      localPrivateKey: localPrivateKey,
      prologue: prologue,
    ),
  );
}
