import 'dart:typed_data';

import '../tx/lab_frame_helper.dart';

enum DtnMessageKind {
  summary(1),
  envelope(2),
  snapshot(3);

  const DtnMessageKind(this.wire);

  final int wire;
}

/// DTN gossip messages: a [LabFrameType.dtn] frame whose payload starts with
/// one [DtnMessageKind] byte. Every request gets one reply of the same kind;
/// an empty envelope or snapshot payload means "nothing to send".
class DtnMessageHelper {
  DtnMessageHelper._();

  static const int idBytes = 32;

  static Uint8List encode(DtnMessageKind kind, List<int> payload) =>
      LabFrameHelper.encode(LabFrameType.dtn, <int>[kind.wire, ...payload]);

  static ({DtnMessageKind kind, Uint8List payload}) decode(
    Uint8List dtnFramePayload,
  ) {
    final kind = dtnFramePayload.isEmpty
        ? null
        : DtnMessageKind.values
              .where((candidate) => candidate.wire == dtnFramePayload[0])
              .firstOrNull;
    if (kind == null) {
      throw const FormatException('Mensagem DTN inválida.');
    }
    return (kind: kind, payload: dtnFramePayload.sublist(1));
  }

  static Uint8List encodeSummary(List<Uint8List> ids) =>
      Uint8List.fromList(<int>[for (final id in ids) ...id]);

  static List<Uint8List> decodeSummary(Uint8List payload) {
    // The engine caps summaries at 256 ids.
    if (payload.length % idBytes != 0 || payload.length > 256 * idBytes) {
      throw const FormatException('Resumo DTN inválido.');
    }
    return <Uint8List>[
      for (var start = 0; start < payload.length; start += idBytes)
        payload.sublist(start, start + idBytes),
    ];
  }
}
