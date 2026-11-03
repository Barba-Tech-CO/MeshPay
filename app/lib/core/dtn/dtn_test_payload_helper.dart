import 'dart:math';
import 'dart:typed_data';

import '../tx/random_bytes_helper.dart';

class DtnTestPayloadHelper {
  DtnTestPayloadHelper._();

  static const int minimumBytes = 1024;
  static const int maximumBytes = 4 * 1024;

  /// Random bytes, 1 to 4 KB, so test envelopes get distinct ids.
  static Uint8List random() => RandomBytesHelper.secure(
    minimumBytes + Random.secure().nextInt(maximumBytes - minimumBytes + 1),
  );
}
