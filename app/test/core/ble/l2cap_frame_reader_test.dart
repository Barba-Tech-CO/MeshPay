import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/l2cap_frame_codec.dart';
import 'package:meshpay/core/ble/l2cap_frame_reader.dart';

List<Uint8List> drain(L2capFrameReader reader) {
  final bodies = <Uint8List>[];
  for (var body = reader.nextBody(); body != null; body = reader.nextBody()) {
    bodies.add(body);
  }
  return bodies;
}

void main() {
  test('reassembles frames split across chunks and merged in one chunk', () {
    final first = Uint8List.fromList(<int>[1, 2, 3]);
    final second = Uint8List.fromList(<int>[4, 5]);
    final stream = Uint8List.fromList(<int>[
      ...L2capFrameCodec.encode(first),
      ...L2capFrameCodec.encode(second),
    ]);
    final reader = L2capFrameReader()..add(stream.sublist(0, 2));

    expect(drain(reader), isEmpty);
    reader.add(stream.sublist(2, 8));
    expect(drain(reader), <Uint8List>[first]);
    reader.add(stream.sublist(8));
    expect(drain(reader), <Uint8List>[second]);
    expect(
      drain(L2capFrameReader()..add(stream)),
      <Uint8List>[first, second],
    );
  });

  test('reassembles a large frame delivered in 8 KB chunks', () {
    final body = Uint8List.fromList(
      List<int>.generate(25 * 1024, (index) => index & 0xff),
    );
    final frame = L2capFrameCodec.encode(body);
    final reader = L2capFrameReader();

    for (var offset = 0; offset < frame.length; offset += 8192) {
      reader.add(
        frame.sublist(
          offset,
          offset + 8192 > frame.length ? frame.length : offset + 8192,
        ),
      );
    }

    expect(drain(reader), <Uint8List>[body]);
  });

  test('rejects an invalid length and drops the buffered bytes', () {
    final reader = L2capFrameReader()..add(Uint8List(4));

    expect(reader.nextBody, throwsFormatException);
    expect(reader.nextBody(), isNull);
  });

  test('rejects a body above the current cap', () {
    final frame = L2capFrameCodec.encode(Uint8List(600));

    expect(
      () => (L2capFrameReader(maxBodyBytes: 512)..add(frame)).nextBody(),
      throwsFormatException,
    );
    final reader = L2capFrameReader(maxBodyBytes: 512)..add(frame);
    reader.maxBodyBytes = L2capFrameCodec.maximumBodyBytes;
    expect(reader.nextBody(), hasLength(600));
  });
}
