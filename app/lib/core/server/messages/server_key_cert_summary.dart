import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';

/// Display fields of a `ServerKeyCert` (envelope tag 1 = `ServerKeyCertBody`).
/// Not a verifier: the core `IssuerTrust` checks the signature.
class ServerKeyCertSummary {
  const ServerKeyCertSummary({required this.keyId, required this.notAfterUnix});

  final Uint8List keyId;
  final int notAfterUnix;

  static ServerKeyCertSummary fromBytes(Uint8List certBytes) {
    var body = Uint8List(0);
    ProtoReaderHelper.read(certBytes, (field, reader) {
      if (field != 1) return false;
      body = reader.readBytes();
      return true;
    });
    var keyId = Uint8List(0);
    var notAfterUnix = 0;
    ProtoReaderHelper.read(body, (field, reader) {
      switch (field) {
        case 1:
          keyId = reader.readBytes();
        case 5:
          notAfterUnix = reader.readUint64().toInt();
        default:
          return false;
      }
      return true;
    });
    return ServerKeyCertSummary(keyId: keyId, notAfterUnix: notAfterUnix);
  }
}
