import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_reader_helper.dart';
import '../codec/proto_writer_helper.dart';

/// Server-sealed cheque; [body] is an encoded `ChequeStubBody`.
class ChequeStub {
  const ChequeStub({required this.body, required this.serverSignature});

  final Uint8List body;
  final Uint8List serverSignature;

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 1, body);
    ProtoWriterHelper.bytes(writer, 2, serverSignature);
    return writer.toBuffer();
  }

  static ChequeStub fromBytes(Uint8List bytes) {
    var body = Uint8List(0);
    var serverSignature = Uint8List(0);
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          body = reader.readBytes();
        case 2:
          serverSignature = reader.readBytes();
        default:
          return false;
      }
      return true;
    });
    return ChequeStub(body: body, serverSignature: serverSignature);
  }
}
