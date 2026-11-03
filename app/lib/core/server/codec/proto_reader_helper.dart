import 'dart:typed_data';

import 'package:protobuf/protobuf.dart';

class ProtoReaderHelper {
  ProtoReaderHelper._();

  /// Calls [onField] for every field; fields it does not consume (returns
  /// false) are skipped, as unknown fields are in proto3.
  static void read(
    Uint8List bytes,
    bool Function(int field, CodedBufferReader reader) onField,
  ) {
    final reader = CodedBufferReader(bytes);
    while (!reader.isAtEnd()) {
      final tag = reader.readTag();
      if (!onField(getTagFieldNumber(tag), reader)) {
        reader.skipField(tag);
      }
    }
  }
}
