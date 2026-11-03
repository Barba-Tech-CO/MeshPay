import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_writer_helper.dart';

/// [body] is the exact `IssueRequestBody` bytes the account signed.
class IssueRequest {
  const IssueRequest({required this.body, required this.signature});

  final Uint8List body;
  final Uint8List signature;

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 1, body);
    ProtoWriterHelper.bytes(writer, 2, signature);
    return writer.toBuffer();
  }
}
