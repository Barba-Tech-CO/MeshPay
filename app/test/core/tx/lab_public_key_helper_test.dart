import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/lab_public_key_helper.dart';

void main() {
  Uint8List key(int length, int prefix) => Uint8List.fromList(<int>[
    prefix,
    ...List<int>.filled(length - 1, 1),
  ]);

  test('accepts compressed 33 and uncompressed 65 byte keys', () {
    expect(LabPublicKeyHelper.isValid(key(33, 2)), isTrue);
    expect(LabPublicKeyHelper.isValid(key(33, 3)), isTrue);
    expect(LabPublicKeyHelper.isValid(key(65, 4)), isTrue);
  });

  test('rejects wrong lengths and mismatched prefixes', () {
    expect(LabPublicKeyHelper.isValid(Uint8List(0)), isFalse);
    expect(LabPublicKeyHelper.isValid(key(32, 2)), isFalse);
    expect(LabPublicKeyHelper.isValid(key(33, 4)), isFalse);
    expect(LabPublicKeyHelper.isValid(key(65, 2)), isFalse);
    expect(LabPublicKeyHelper.isValid(key(64, 4)), isFalse);
  });
}
