import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';
import '../messages/cheque_stub.dart';

class IssueResponse {
  const IssueResponse({
    required this.chequebookId,
    required this.stubs,
    required this.reserveUnits,
  });

  final Uint8List chequebookId;
  final List<ChequeStub> stubs;
  final int reserveUnits;

  static IssueResponse fromBytes(Uint8List bytes) {
    var chequebookId = Uint8List(0);
    final stubs = <ChequeStub>[];
    var reserveUnits = 0;
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          chequebookId = reader.readBytes();
        case 2:
          stubs.add(ChequeStub.fromBytes(reader.readBytes()));
        case 3:
          reserveUnits = reader.readUint64().toInt();
        default:
          return false;
      }
      return true;
    });
    return IssueResponse(
      chequebookId: chequebookId,
      stubs: List<ChequeStub>.unmodifiable(stubs),
      reserveUnits: reserveUnits,
    );
  }
}
