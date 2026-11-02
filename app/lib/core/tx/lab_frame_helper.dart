import 'dart:typed_data';

import 'tx_error_code.dart';

/// Mandatory first byte of every application frame after the handshake, so
/// payload content can never be mistaken for the frame type.
enum LabFrameType {
  echo(1),
  tx(2),
  error(3);

  const LabFrameType(this.wire);

  final int wire;
}

class LabFrameHelper {
  LabFrameHelper._();

  static Uint8List encode(LabFrameType type, List<int> payload) =>
      Uint8List.fromList(<int>[type.wire, ...payload]);

  static Uint8List encodeError(TxErrorCode code) =>
      encode(LabFrameType.error, <int>[code.wire]);

  static ({LabFrameType type, Uint8List payload}) decode(Uint8List plaintext) {
    final type = plaintext.isEmpty
        ? null
        : LabFrameType.values
              .where((candidate) => candidate.wire == plaintext[0])
              .firstOrNull;
    if (type == null) {
      throw const FormatException('Tipo de quadro desconhecido.');
    }
    return (type: type, payload: plaintext.sublist(1));
  }

  static TxErrorCode errorCodeOf(Uint8List errorPayload) => errorPayload.isEmpty
      ? TxErrorCode.internal
      : TxErrorCode.fromWire(errorPayload[0]);
}
