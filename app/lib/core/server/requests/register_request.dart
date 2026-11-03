import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_writer_helper.dart';
import '../messages/server_attestation_level.dart';

class RegisterRequest {
  const RegisterRequest({
    required this.publicKey,
    required this.signature,
    required this.attestationLevel,
    required this.attestationChain,
    required this.challenge,
  });

  final Uint8List publicKey;

  /// Raw r||s over the register message.
  final Uint8List signature;

  /// Client claim only; the server derives the level from the chain.
  final ServerAttestationLevel attestationLevel;

  /// DER certificates, leaf first; empty for a lab software registration.
  final List<Uint8List> attestationChain;

  /// Nonce from `GET /v1/challenge`; empty without a chain.
  final Uint8List challenge;

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 1, publicKey);
    ProtoWriterHelper.bytes(writer, 2, signature);
    ProtoWriterHelper.enumeration(writer, 3, attestationLevel.value);
    for (final certificate in attestationChain) {
      writer.writeField(4, PbFieldType.OY, certificate);
    }
    ProtoWriterHelper.bytes(writer, 5, challenge);
    return writer.toBuffer();
  }
}
