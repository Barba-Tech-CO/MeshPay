import 'dart:typed_data';

import '../../../../core/tx/lab_frame_helper.dart';

class L2capPayerEchoHelper {
  L2capPayerEchoHelper._();

  static const int bodyBytes = 25 * 1024;
  static const int repetitions = 5;

  static Uint8List frame() => LabFrameHelper.encode(
    LabFrameType.echo,
    Uint8List(bodyBytes)..fillRange(0, bodyBytes, 0x5a),
  );
}
