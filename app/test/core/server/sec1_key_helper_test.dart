import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/server/signing/sec1_key_helper.dart';

void main() {
  Uint8List point(int lastY) => Uint8List.fromList(<int>[
    4,
    ...List<int>.filled(32, 7),
    ...List<int>.filled(31, 8),
    lastY,
  ]);

  test('compresses by Y parity', () {
    expect(Sec1KeyHelper.compress(point(2)), <int>[
      2,
      ...List<int>.filled(32, 7),
    ]);
    expect(Sec1KeyHelper.compress(point(5)), <int>[
      3,
      ...List<int>.filled(32, 7),
    ]);
  });

  test('keeps a compressed key and rejects garbage', () {
    final compressed = Uint8List.fromList(<int>[2, ...List<int>.filled(32, 1)]);

    expect(Sec1KeyHelper.compress(compressed), compressed);
    expect(() => Sec1KeyHelper.compress(Uint8List(10)), throwsFormatException);
  });
}
