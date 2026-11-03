import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/dtn_message_helper.dart';
import 'package:meshpay/core/tx/lab_frame_helper.dart';

void main() {
  test('round-trips a DTN message inside a type 4 frame', () {
    final plaintext = DtnMessageHelper.encode(
      DtnMessageKind.envelope,
      <int>[7, 8],
    );

    final frame = LabFrameHelper.decode(plaintext);
    final message = DtnMessageHelper.decode(frame.payload);

    expect(frame.type, LabFrameType.dtn);
    expect(plaintext[0], 4);
    expect(message.kind, DtnMessageKind.envelope);
    expect(message.payload, <int>[7, 8]);
  });

  test('kinds have explicit wire bytes, not enum indexes', () {
    expect(
      DtnMessageKind.values.map((kind) => kind.wire),
      <int>[1, 2, 3],
    );
    expect(
      DtnMessageHelper.encode(DtnMessageKind.summary, const <int>[])[1],
      1,
    );
    expect(
      DtnMessageHelper.decode(Uint8List.fromList(<int>[3])).kind,
      DtnMessageKind.snapshot,
    );
    expect(
      () => DtnMessageHelper.decode(Uint8List.fromList(<int>[0])),
      throwsFormatException,
    );
  });

  test('still rejects unknown frame types and unknown DTN kinds', () {
    expect(
      () => LabFrameHelper.decode(Uint8List.fromList(<int>[5, 1])),
      throwsFormatException,
    );
    expect(
      () => DtnMessageHelper.decode(Uint8List.fromList(<int>[9])),
      throwsFormatException,
    );
    expect(() => DtnMessageHelper.decode(Uint8List(0)), throwsFormatException);
  });

  test('summaries are whole 32-byte ids, at most 256', () {
    final ids = <Uint8List>[Uint8List(32), Uint8List(32)..[0] = 1];

    final decoded = DtnMessageHelper.decodeSummary(
      DtnMessageHelper.encodeSummary(ids),
    );

    expect(decoded, ids);
    expect(
      () => DtnMessageHelper.decodeSummary(Uint8List(33)),
      throwsFormatException,
    );
    expect(
      () => DtnMessageHelper.decodeSummary(Uint8List(257 * 32)),
      throwsFormatException,
    );
  });
}
