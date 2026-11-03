import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../codec/proto_writer_helper.dart';

class ChequeSpendBody {
  const ChequeSpendBody({
    required this.chequeId,
    required this.payeeAccountId,
    required this.amountUnits,
    required this.txId,
    required this.counter,
    required this.prevSpendHash,
    required this.signedAtUnix,
  });

  final Uint8List chequeId;
  final Uint8List payeeAccountId;
  final int amountUnits;

  /// Empty while deriving the tx_id (the server hashes the body without it).
  final Uint8List txId;
  final int counter;
  final Uint8List prevSpendHash;
  final int signedAtUnix;

  ChequeSpendBody withTxId(Uint8List value) => ChequeSpendBody(
    chequeId: chequeId,
    payeeAccountId: payeeAccountId,
    amountUnits: amountUnits,
    txId: value,
    counter: counter,
    prevSpendHash: prevSpendHash,
    signedAtUnix: signedAtUnix,
  );

  Uint8List toBytes() {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 1, chequeId);
    ProtoWriterHelper.bytes(writer, 2, payeeAccountId);
    ProtoWriterHelper.uint64(writer, 3, amountUnits);
    ProtoWriterHelper.bytes(writer, 4, txId);
    ProtoWriterHelper.uint64(writer, 5, counter);
    ProtoWriterHelper.bytes(writer, 6, prevSpendHash);
    ProtoWriterHelper.uint64(writer, 7, signedAtUnix);
    return writer.toBuffer();
  }
}
