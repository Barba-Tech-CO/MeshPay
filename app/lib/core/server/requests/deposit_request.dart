import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_writer_helper.dart';
import '../messages/cheque_spend.dart';
import '../messages/cheque_stub.dart';

class DepositRequest {
  const DepositRequest({required this.stub, required this.spend});

  final ChequeStub stub;
  final ChequeSpend spend;

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    // Embedded messages share the length-delimited wire type with bytes.
    ProtoWriterHelper.bytes(writer, 1, stub.toBytes());
    ProtoWriterHelper.bytes(writer, 2, spend.toBytes());
    return writer.toBuffer();
  }
}
