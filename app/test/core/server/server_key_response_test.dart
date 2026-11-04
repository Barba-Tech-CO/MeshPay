import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/server/codec/proto_writer_helper.dart';
import 'package:meshpay/core/server/responses/server_key_response.dart';
import 'package:protobuf/protobuf.dart';

void main() {
  test('parses snapshot cert from server-key field 5', () {
    final writer = CodedBufferWriter();
    ProtoWriterHelper.bytes(writer, 3, Uint8List.fromList(<int>[1]));
    ProtoWriterHelper.bytes(writer, 4, Uint8List.fromList(<int>[2]));
    ProtoWriterHelper.bytes(writer, 5, Uint8List.fromList(<int>[3, 4]));

    final response = ServerKeyResponse.fromBytes(
      Uint8List.fromList(writer.toBuffer()),
    );

    expect(response.snapshotCert, <int>[3, 4]);
  });
}
