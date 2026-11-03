import 'dart:typed_data';

import '../codec/proto_reader_helper.dart';

class RegisterResponse {
  const RegisterResponse({
    required this.accountId,
    required this.epoch,
    required this.created,
    required this.availableUnits,
  });

  final Uint8List accountId;
  final int epoch;
  final bool created;
  final int availableUnits;

  static RegisterResponse fromBytes(Uint8List bytes) {
    var accountId = Uint8List(0);
    var epoch = 0;
    var created = false;
    var availableUnits = 0;
    ProtoReaderHelper.read(bytes, (field, reader) {
      switch (field) {
        case 1:
          accountId = reader.readBytes();
        case 2:
          epoch = reader.readUint64().toInt();
        case 3:
          created = reader.readBool();
        case 4:
          availableUnits = reader.readUint64().toInt();
        default:
          return false;
      }
      return true;
    });
    return RegisterResponse(
      accountId: accountId,
      epoch: epoch,
      created: created,
      availableUnits: availableUnits,
    );
  }
}
