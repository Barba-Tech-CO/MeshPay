import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';

/// `GET /v1/server-key`. Only the trust fields the receiver uses are kept.
class ServerKeyResponse {
  ServerKeyResponse({
    required this.rootPublicKey,
    required this.issuerCert,
    Uint8List? snapshotCert,
  }) : snapshotCert = snapshotCert ?? Uint8List(0);

  final Uint8List rootPublicKey;
  final Uint8List issuerCert;
  final Uint8List snapshotCert;

  static ServerKeyResponse fromBytes(Uint8List bytes) {
    var rootPublicKey = Uint8List(0);
    var issuerCert = Uint8List(0);
    var snapshotCert = Uint8List(0);
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 3:
          rootPublicKey = reader.readBytes();
        case 4:
          issuerCert = reader.readBytes();
        case 5:
          snapshotCert = reader.readBytes();
        default:
          return false;
      }
      return true;
    });
    return ServerKeyResponse(
      rootPublicKey: rootPublicKey,
      issuerCert: issuerCert,
      snapshotCert: snapshotCert,
    );
  }
}
