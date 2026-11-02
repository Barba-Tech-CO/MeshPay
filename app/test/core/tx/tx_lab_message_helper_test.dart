import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/lab_frame_helper.dart';
import 'package:meshpay/core/tx/tx_error_code.dart';
import 'package:meshpay/core/tx/tx_lab_message_helper.dart';

void main() {
  test('the frame type is the first byte, whatever the payload holds', () {
    final echo = LabFrameHelper.encode(LabFrameType.echo, <int>[2, 2, 2]);
    final tx = TxLabMessageHelper.encode(TxLabMessageKind.propose, <int>[1]);

    expect(echo, <int>[1, 2, 2, 2]);
    expect(LabFrameHelper.decode(echo).type, LabFrameType.echo);
    final frame = LabFrameHelper.decode(tx);
    expect(frame.type, LabFrameType.tx);
    final message = TxLabMessageHelper.decode(frame.payload);
    expect(message.kind, TxLabMessageKind.propose);
    expect(message.payload, <int>[1]);
  });

  test('error frames carry a fixed code', () {
    final frame = LabFrameHelper.decode(
      LabFrameHelper.encodeError(TxErrorCode.notAddressedToUs),
    );

    expect(frame.type, LabFrameType.error);
    expect(
      LabFrameHelper.errorCodeOf(frame.payload),
      TxErrorCode.notAddressedToUs,
    );
  });

  test('rejects empty frames, unknown types and unknown message kinds', () {
    expect(() => LabFrameHelper.decode(Uint8List(0)), throwsFormatException);
    expect(
      () => LabFrameHelper.decode(Uint8List.fromList(<int>[9, 1])),
      throwsFormatException,
    );
    expect(
      () => TxLabMessageHelper.decode(Uint8List.fromList(<int>[99])),
      throwsFormatException,
    );
  });
}
