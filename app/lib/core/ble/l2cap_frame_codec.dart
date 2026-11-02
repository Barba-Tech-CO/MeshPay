import 'dart:typed_data';

class L2capFrameCodec {
  L2capFrameCodec._();

  static const int _headerLength = 4;
  static const int maximumBodyBytes = 64 * 1024;

  static Uint8List encode(Uint8List body) {
    if (body.isEmpty || body.length > maximumBodyBytes) {
      throw FormatException(
        'An L2CAP frame body must contain between 1 and '
        '$maximumBodyBytes bytes.',
      );
    }

    final frame = Uint8List(_headerLength + body.length);
    ByteData.sublistView(frame).setUint32(0, body.length, Endian.big);
    frame.setRange(_headerLength, frame.length, body);
    return frame;
  }

  static Uint8List decode(Uint8List frame) {
    if (frame.length < _headerLength) {
      throw const FormatException('The L2CAP frame header is incomplete.');
    }

    final bodyLength = ByteData.sublistView(
      frame,
      0,
      _headerLength,
    ).getUint32(0, Endian.big);
    if (bodyLength == 0 || bodyLength > maximumBodyBytes) {
      throw FormatException(
        'The L2CAP frame body length must be between 1 and '
        '$maximumBodyBytes bytes.',
      );
    }
    if (frame.length != _headerLength + bodyLength) {
      throw const FormatException('The L2CAP frame length does not match.');
    }

    return Uint8List.fromList(frame.sublist(_headerLength));
  }
}
