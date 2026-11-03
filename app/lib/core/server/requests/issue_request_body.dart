import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_writer_helper.dart';

class IssueRequestBody {
  const IssueRequestBody({
    required this.accountId,
    required this.requestId,
    required this.chequeCount,
    required this.maxAmountUnits,
    required this.ttlSeconds,
  });

  final Uint8List accountId;
  final Uint8List requestId;
  final int chequeCount;
  final int maxAmountUnits;
  final int ttlSeconds;

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 1, accountId);
    ProtoWriterHelper.bytes(writer, 2, requestId);
    ProtoWriterHelper.uint32(writer, 3, chequeCount);
    ProtoWriterHelper.uint64(writer, 4, maxAmountUnits);
    ProtoWriterHelper.uint64(writer, 5, ttlSeconds);
    return writer.toBuffer();
  }
}
