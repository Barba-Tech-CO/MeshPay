import 'dart:typed_data';

/// Minimal protobuf encoding/decoding for the lab cheque messages
/// (`ChequeSpendBody`, `TxPropose`, `AckReceipt`; see core/src/tx_wire).
class TxWireHelper {
  TxWireHelper._();

  static const int _lengthDelimited = 2;
  static const int _maxVarintBytes = 10;

  static Uint8List encodeBytesField(int tag, List<int> value) =>
      Uint8List.fromList(<int>[
        tag << 3 | _lengthDelimited,
        ..._encodeVarint(value.length),
        ...value,
      ]);

  /// `TxPropose.spend(2).body(1).tx_id(4)`.
  static Uint8List? txIdFromPropose(Uint8List propose) {
    final spend = _field(propose, 2);
    final body = spend == null ? null : _field(spend, 1);
    return body == null ? null : _field(body, 4);
  }

  /// `AckReceipt.tx_id(1)`.
  static Uint8List? txIdFromAck(Uint8List ack) => _field(ack, 1);

  static List<int> _encodeVarint(int value) {
    final bytes = <int>[];
    var rest = value;
    while (rest >= 0x80) {
      bytes.add(rest & 0x7f | 0x80);
      rest >>= 7;
    }
    return bytes..add(rest);
  }

  static Uint8List? _field(Uint8List bytes, int tag) {
    var offset = 0;
    while (offset < bytes.length) {
      final (key, afterKey) = _readVarint(bytes, offset);
      offset = afterKey;
      switch (key & 7) {
        case 0:
          offset = _readVarint(bytes, offset).$2;
        case 1:
          offset = _skip(bytes, offset, 8);
        case _lengthDelimited:
          final (length, afterLength) = _readVarint(bytes, offset);
          offset = _skip(bytes, afterLength, length);
          if (key >> 3 == tag) {
            return bytes.sublist(afterLength, offset);
          }
        case 5:
          offset = _skip(bytes, offset, 4);
        default:
          throw const FormatException('Unsupported protobuf wire type.');
      }
    }
    return null;
  }

  static int _skip(Uint8List bytes, int offset, int length) {
    if (length < 0 || length > bytes.length - offset) {
      throw const FormatException('Truncated protobuf field.');
    }
    return offset + length;
  }

  /// At most [_maxVarintBytes] bytes; a longer varint is malformed.
  static (int, int) _readVarint(Uint8List bytes, int start) {
    var value = 0;
    for (var index = 0; index < _maxVarintBytes; index++) {
      final offset = start + index;
      if (offset >= bytes.length) {
        break;
      }
      value |= (bytes[offset] & 0x7f) << (7 * index);
      if (bytes[offset] & 0x80 == 0) {
        return (value, offset + 1);
      }
    }
    throw const FormatException('Malformed protobuf varint.');
  }
}
