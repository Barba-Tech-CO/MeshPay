import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/l2cap_frame_codec.dart';

void main() {
  test('encodes a four-byte big-endian length and decodes the body', () {
    final body = Uint8List.fromList(<int>[1, 2, 3, 4]);

    final frame = L2capFrameCodec.encode(body);

    expect(frame.sublist(0, 4), <int>[0, 0, 0, 4]);
    expect(L2capFrameCodec.decode(frame), body);
  });

  test('round-trips the 25 KB benchmark body', () {
    final body = Uint8List(25 * 1024)..fillRange(0, 25 * 1024, 0x5a);

    expect(L2capFrameCodec.decode(L2capFrameCodec.encode(body)), body);
  });

  test('rejects empty and oversized bodies when encoding', () {
    expect(
      () => L2capFrameCodec.encode(Uint8List(0)),
      throwsA(isA<FormatException>()),
    );
    expect(
      () => L2capFrameCodec.encode(Uint8List(64 * 1024 + 1)),
      throwsA(isA<FormatException>()),
    );
  });

  test('rejects invalid lengths and incomplete frames when decoding', () {
    expect(
      () => L2capFrameCodec.decode(Uint8List(3)),
      throwsA(isA<FormatException>()),
    );
    expect(
      () => L2capFrameCodec.decode(Uint8List.fromList(<int>[0, 0, 0, 0])),
      throwsA(isA<FormatException>()),
    );
    expect(
      () => L2capFrameCodec.decode(
        Uint8List.fromList(<int>[0, 1, 0, 1]),
      ),
      throwsA(isA<FormatException>()),
    );
    expect(
      () => L2capFrameCodec.decode(
        Uint8List.fromList(<int>[0, 0, 0, 2, 1]),
      ),
      throwsA(isA<FormatException>()),
    );
    expect(
      () => L2capFrameCodec.decode(
        Uint8List.fromList(<int>[0, 0, 0, 1, 1, 2]),
      ),
      throwsA(isA<FormatException>()),
    );
  });

  test('accepts a body of exactly 64 KB', () {
    final body = Uint8List(64 * 1024)..fillRange(0, 64 * 1024, 0xa5);

    expect(L2capFrameCodec.decode(L2capFrameCodec.encode(body)), body);
  });
}
