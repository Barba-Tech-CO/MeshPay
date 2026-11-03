import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';
import '../messages/redeem_status.dart';

/// The equivocation proof (field 3) is only flagged; the lab does not use it.
class DepositResponse {
  const DepositResponse({
    required this.status,
    required this.hasProof,
    required this.creditedUnits,
  });

  final RedeemStatus status;
  final bool hasProof;
  final int creditedUnits;

  static DepositResponse fromBytes(Uint8List bytes) {
    var status = RedeemStatus.unspecified;
    var hasProof = false;
    var creditedUnits = 0;
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          status = RedeemStatus.fromValue(reader.readEnum());
        case 3:
          reader.readBytes();
          hasProof = true;
        case 4:
          creditedUnits = reader.readUint64().toInt();
        default:
          // Field 2 (detail) is server free text: never shown, skipped.
          return false;
      }
      return true;
    });
    return DepositResponse(
      status: status,
      hasProof: hasProof,
      creditedUnits: creditedUnits,
    );
  }
}
