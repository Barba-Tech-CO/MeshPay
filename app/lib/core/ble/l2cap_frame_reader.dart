import 'dart:typed_data';

import 'l2cap_frame_codec.dart';

/// Reassembles length-prefixed frames from arbitrary stream chunks.
class L2capFrameReader {
  L2capFrameReader({this.maxBodyBytes = L2capFrameCodec.maximumBodyBytes});

  /// Largest body accepted when the next header is parsed.
  int maxBodyBytes;
  Uint8List _buffer = Uint8List(0);
  int _length = 0;

  void add(Uint8List chunk) {
    final needed = _length + chunk.length;
    if (needed > _buffer.length) {
      final grown = Uint8List(
        needed > _buffer.length * 2 ? needed : _buffer.length * 2,
      )..setRange(0, _length, _buffer);
      _buffer = grown;
    }
    _buffer.setRange(_length, needed, chunk);
    _length = needed;
  }

  /// Next complete body, or null while more bytes are needed. Throws
  /// [FormatException] (and drops the buffer) on an invalid length.
  Uint8List? nextBody() {
    if (!L2capFrameCodec.hasHeader(_length)) {
      return null;
    }
    final int bodyLength;
    try {
      bodyLength = L2capFrameCodec.bodyLengthOf(
        Uint8List.sublistView(_buffer, 0, _length),
        maxBodyBytes: maxBodyBytes,
      );
    } on FormatException {
      _length = 0;
      rethrow;
    }
    final frameLength = L2capFrameCodec.frameLengthOf(bodyLength);
    if (_length < frameLength) {
      return null;
    }
    final body = L2capFrameCodec.decode(
      Uint8List.sublistView(_buffer, 0, frameLength),
    );
    _buffer.setRange(0, _length - frameLength, _buffer, frameLength);
    _length -= frameLength;
    return body;
  }
}
