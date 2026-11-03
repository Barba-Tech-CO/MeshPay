import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';

class BalanceResponse {
  const BalanceResponse({
    required this.availableUnits,
    required this.reservedUnits,
    required this.frozen,
  });

  final int availableUnits;
  final int reservedUnits;
  final bool frozen;

  static BalanceResponse fromBytes(Uint8List bytes) {
    var availableUnits = 0;
    var reservedUnits = 0;
    var frozen = false;
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          availableUnits = reader.readUint64().toInt();
        case 2:
          reservedUnits = reader.readUint64().toInt();
        case 3:
          frozen = reader.readBool();
        default:
          return false;
      }
      return true;
    });
    return BalanceResponse(
      availableUnits: availableUnits,
      reservedUnits: reservedUnits,
      frozen: frozen,
    );
  }
}
