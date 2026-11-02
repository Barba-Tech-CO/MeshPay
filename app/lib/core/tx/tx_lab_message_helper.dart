import 'dart:typed_data';

import 'lab_frame_helper.dart';

enum TxLabMessageKind { hello, helloReply, propose, ack }

/// Transaction messages: a [LabFrameType.tx] frame whose payload starts with
/// one [TxLabMessageKind] byte.
class TxLabMessageHelper {
  TxLabMessageHelper._();

  static Uint8List encode(TxLabMessageKind kind, List<int> payload) =>
      LabFrameHelper.encode(LabFrameType.tx, <int>[kind.index, ...payload]);

  static ({TxLabMessageKind kind, Uint8List payload}) decode(
    Uint8List txFramePayload,
  ) {
    if (txFramePayload.isEmpty ||
        txFramePayload[0] >= TxLabMessageKind.values.length) {
      throw const FormatException('Mensagem de transação inválida.');
    }
    return (
      kind: TxLabMessageKind.values[txFramePayload[0]],
      payload: txFramePayload.sublist(1),
    );
  }
}
