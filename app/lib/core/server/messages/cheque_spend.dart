import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_writer_helper.dart';

/// [body] is an encoded `ChequeSpendBody`; [payerSignature] is raw r||s.
class ChequeSpend {
  const ChequeSpend({required this.body, required this.payerSignature});

  final Uint8List body;
  final Uint8List payerSignature;

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 1, body);
    ProtoWriterHelper.bytes(writer, 2, payerSignature);
    return writer.toBuffer();
  }
}
