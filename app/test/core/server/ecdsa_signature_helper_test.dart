import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/noise/hex_helper.dart';
import 'package:meshpay/core/server/signing/ecdsa_signature_helper.dart';

// n - 1 of P-256: a high S that must be flipped to 1.
const _highS =
    'ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632550';

Uint8List _der(List<int> r, List<int> s) => Uint8List.fromList(<int>[
  0x30,
  4 + r.length + s.length,
  0x02,
  r.length,
  ...r,
  0x02,
  s.length,
  ...s,
]);

void main() {
  test('strips the sign byte and left-pads to 32 bytes', () {
    final r = <int>[0x00, 0x80, ...List<int>.filled(31, 1)];
    final raw = EcdsaSignatureHelper.derToRawLowS(_der(r, <int>[0x05]));

    expect(raw, hasLength(64));
    expect(raw.sublist(0, 32), <int>[0x80, ...List<int>.filled(31, 1)]);
    expect(raw.sublist(32), <int>[...List<int>.filled(31, 0), 0x05]);
  });

  test('normalizes a high S to n - s', () {
    final raw = EcdsaSignatureHelper.derToRawLowS(
      _der(<int>[0x07], <int>[0x00, ...HexHelper.decode(_highS)]),
    );

    expect(raw.sublist(32), <int>[...List<int>.filled(31, 0), 0x01]);
  });

  test('rejects malformed DER', () {
    expect(
      () => EcdsaSignatureHelper.derToRawLowS(Uint8List.fromList(<int>[1, 2])),
      throwsFormatException,
    );
    expect(
      () => EcdsaSignatureHelper.derToRawLowS(_der(<int>[0x00], <int>[0x01])),
      throwsFormatException,
    );
  });
}
