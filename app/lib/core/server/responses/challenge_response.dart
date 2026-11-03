import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';

class ChallengeResponse {
  const ChallengeResponse({required this.nonce, required this.expiresAtUnix});

  final Uint8List nonce;
  final int expiresAtUnix;

  static ChallengeResponse fromBytes(Uint8List bytes) {
    var nonce = Uint8List(0);
    var expiresAtUnix = 0;
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          nonce = reader.readBytes();
        case 2:
          expiresAtUnix = reader.readUint64().toInt();
        default:
          return false;
      }
      return true;
    });
    return ChallengeResponse(nonce: nonce, expiresAtUnix: expiresAtUnix);
  }
}
