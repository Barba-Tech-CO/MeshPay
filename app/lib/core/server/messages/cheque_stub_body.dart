import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';
import 'server_attestation_level.dart';

class ChequeStubBody {
  const ChequeStubBody({
    required this.chequeId,
    required this.chequebookId,
    required this.payerAccountId,
    required this.payerEpoch,
    required this.signerPubkey,
    required this.maxAmountUnits,
    required this.expiresAtUnix,
    required this.attestationLevel,
    required this.singleUseKey,
    required this.serverKeyId,
  });

  final Uint8List chequeId;
  final Uint8List chequebookId;
  final Uint8List payerAccountId;
  final int payerEpoch;
  final Uint8List signerPubkey;
  final int maxAmountUnits;
  final int expiresAtUnix;
  final ServerAttestationLevel attestationLevel;
  final bool singleUseKey;
  final Uint8List serverKeyId;

  static ChequeStubBody fromBytes(Uint8List bytes) {
    var chequeId = Uint8List(0);
    var chequebookId = Uint8List(0);
    var payerAccountId = Uint8List(0);
    var payerEpoch = 0;
    var signerPubkey = Uint8List(0);
    var maxAmountUnits = 0;
    var expiresAtUnix = 0;
    var attestationLevel = ServerAttestationLevel.unspecified;
    var singleUseKey = false;
    var serverKeyId = Uint8List(0);
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          chequeId = reader.readBytes();
        case 2:
          chequebookId = reader.readBytes();
        case 3:
          payerAccountId = reader.readBytes();
        case 4:
          payerEpoch = reader.readUint64().toInt();
        case 5:
          signerPubkey = reader.readBytes();
        case 6:
          maxAmountUnits = reader.readUint64().toInt();
        case 7:
          expiresAtUnix = reader.readUint64().toInt();
        case 8:
          attestationLevel = ServerAttestationLevel.fromValue(
            reader.readEnum(),
          );
        case 9:
          singleUseKey = reader.readBool();
        case 10:
          serverKeyId = reader.readBytes();
        default:
          return false;
      }
      return true;
    });
    return ChequeStubBody(
      chequeId: chequeId,
      chequebookId: chequebookId,
      payerAccountId: payerAccountId,
      payerEpoch: payerEpoch,
      signerPubkey: signerPubkey,
      maxAmountUnits: maxAmountUnits,
      expiresAtUnix: expiresAtUnix,
      attestationLevel: attestationLevel,
      singleUseKey: singleUseKey,
      serverKeyId: serverKeyId,
    );
  }
}
