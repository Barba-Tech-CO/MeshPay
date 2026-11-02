import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/noise/hex_helper.dart';
import 'package:meshpay/core/noise/noise_prologue_helper.dart';

void main() {
  const session = '0123456789abcdef';
  final key = 'ab' * 32;

  test('prologue is the deterministic utf8 of the QR data', () {
    final prologue = NoisePrologueHelper.build(
      sessionIdHex: session,
      psm: 25,
      keyHex: key,
    );

    expect(String.fromCharCodes(prologue), 'meshpay/1|$session|25|$key');
    expect(
      NoisePrologueHelper.build(sessionIdHex: session, psm: 25, keyHex: key),
      prologue,
    );
  });

  test('prologue changes when any QR field changes', () {
    final base = NoisePrologueHelper.build(
      sessionIdHex: session,
      psm: 25,
      keyHex: key,
    );

    expect(
      NoisePrologueHelper.build(
        sessionIdHex: '0123456789abcdee',
        psm: 25,
        keyHex: key,
      ),
      isNot(base),
    );
    expect(
      NoisePrologueHelper.build(sessionIdHex: session, psm: 26, keyHex: key),
      isNot(base),
    );
    expect(
      NoisePrologueHelper.build(
        sessionIdHex: session,
        psm: 25,
        keyHex: 'ac${'ab' * 31}',
      ),
      isNot(base),
    );
  });

  test('hex helper round-trips and validates length and alphabet', () {
    final bytes = HexHelper.decode(key);

    expect(bytes, hasLength(32));
    expect(HexHelper.encode(bytes), key);
    expect(HexHelper.isValid(key, 64), isTrue);
    expect(HexHelper.isValid(key, 62), isFalse);
    expect(HexHelper.isValid('zz${'ab' * 31}', 64), isFalse);
  });
}
