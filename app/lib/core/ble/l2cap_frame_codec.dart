import 'dart:typed_data';

class L2capFrameCodec {
  L2capFrameCodec._();

  static const int _headerLength = 4;
  static const int maximumBodyBytes = 64 * 1024;

  static bool hasHeader(int availableBytes) => availableBytes >= _headerLength;

  static int frameLengthOf(int bodyLength) => _headerLength + bodyLength;

  /// Reads and validates the body length from the first four bytes.
  static int bodyLengthOf(
    Uint8List header, {
    int maxBodyBytes = maximumBodyBytes,
  }) {
    if (!hasHeader(header.length)) {
      throw const FormatException('The L2CAP frame header is incomplete.');
    }
    final bodyLength = ByteData.sublistView(
      header,
      0,
      _headerLength,
    ).getUint32(0, Endian.big);
    if (bodyLength == 0 || bodyLength > maxBodyBytes) {
      throw FormatException(
        'The L2CAP frame body length must be between 1 and '
        '$maxBodyBytes bytes.',
      );
    }
    return bodyLength;
  }

  static Uint8List encode(Uint8List body) {
    if (body.isEmpty || body.length > maximumBodyBytes) {
      throw FormatException(
        'An L2CAP frame body must contain between 1 and '
        '$maximumBodyBytes bytes.',
      );
    }

    final frame = Uint8List(frameLengthOf(body.length));
    ByteData.sublistView(frame).setUint32(0, body.length, Endian.big);
    frame.setRange(_headerLength, frame.length, body);
    return frame;
  }

  static Uint8List decode(Uint8List frame) {
    final bodyLength = bodyLengthOf(frame);
    if (frame.length != frameLengthOf(bodyLength)) {
      throw const FormatException('The L2CAP frame length does not match.');
    }

    return Uint8List.fromList(frame.sublist(_headerLength));
  }
}
