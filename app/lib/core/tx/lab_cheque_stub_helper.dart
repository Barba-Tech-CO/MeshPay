import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

import '../server/codec/proto_writer_helper.dart';
import '../server/messages/cheque_stub.dart';

/// LAB ONLY: the lab has no issuing server, so the payer builds the
/// `ChequeStub` itself (unsigned; the receiver does not verify the ISSUER
/// signature yet). [expirySeconds] is `E` on both sides.
class LabChequeStubHelper {
  LabChequeStubHelper._();

  static const int expirySeconds = 72 * 3600;

  /// Stub for [chequeId] expiring `E` after [issuedAtUnix].
  static Uint8List forCheque(Uint8List chequeId, int issuedAtUnix) {
    final body = CodedBufferWriter();
    ProtoWriterHelper.bytes(body, 1, chequeId);
    ProtoWriterHelper.uint64(body, 7, issuedAtUnix + expirySeconds);
    return ChequeStub(
      body: body.toBuffer(),
      serverSignature: Uint8List(0),
    ).toBytes();
  }
}
