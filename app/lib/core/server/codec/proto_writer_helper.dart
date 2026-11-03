import 'package:fixnum/fixnum.dart';
import 'package:protobuf/protobuf.dart';

/// proto3 field writers that skip default values, as prost does, so the
/// output is byte-identical to the server encoding.
class ProtoWriterHelper {
  ProtoWriterHelper._();

  static void bytes(CodedBufferWriter writer, int field, List<int> value) {
    if (value.isNotEmpty) {
      writer.writeField(field, PbFieldType.OY, value);
    }
  }

  static void uint64(CodedBufferWriter writer, int field, int value) {
    if (value != 0) {
      writer.writeField(field, PbFieldType.OU6, Int64(value));
    }
  }

  static void uint32(CodedBufferWriter writer, int field, int value) {
    if (value != 0) {
      writer.writeField(field, PbFieldType.OU3, value);
    }
  }

  /// Enumerations are int32 varints on the wire.
  static void enumeration(CodedBufferWriter writer, int field, int value) {
    if (value != 0) {
      writer.writeField(field, PbFieldType.O3, value);
    }
  }
}
