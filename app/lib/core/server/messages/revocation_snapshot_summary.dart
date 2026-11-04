import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';

/// Display fields of a `RevocationSnapshot`; the signature is not checked here.
class RevocationSnapshotSummary {
  const RevocationSnapshotSummary({
    required this.issuedAtUnix,
    required this.revokedServerKeyCount,
  });

  final int issuedAtUnix;
  final int revokedServerKeyCount;

  static RevocationSnapshotSummary fromBytes(Uint8List bytes) {
    var issuedAtUnix = 0;
    var revokedServerKeyCount = 0;
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 2:
          issuedAtUnix = reader.readUint64().toInt();
        case 9:
          reader.readBytes();
          revokedServerKeyCount++;
        default:
          return false;
      }
      return true;
    });
    return RevocationSnapshotSummary(
      issuedAtUnix: issuedAtUnix,
      revokedServerKeyCount: revokedServerKeyCount,
    );
  }
}
