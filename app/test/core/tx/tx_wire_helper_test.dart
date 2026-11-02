import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';

void main() {
  test('builds a spend body with multi-byte varints and no tx_id', () {
    final body = TxWireHelper.buildSpendBody(
      chequeId: Uint8List.fromList(<int>[1, 2]),
      payeeAccountId: Uint8List.fromList(<int>[3]),
      amountUnits: 300,
      counter: 1,
      signedAtUnix: 0,
    );

    expect(body, <int>[
      0x0a, 2, 1, 2, // cheque_id
      0x12, 1, 3, // payee_account_id
      0x18, 0xac, 0x02, // amount_units = 300
      0x28, 1, // counter
    ]);
  });

  test('reads the tx_id of a propose and of an ack', () {
    final txId = Uint8List.fromList(List<int>.generate(32, (i) => i));
    final body = <int>[
      0x18,
      0x05,
      ...TxWireHelper.encodeBytesField(4, txId),
    ];
    final propose = TxWireHelper.encodeBytesField(
      2,
      TxWireHelper.encodeBytesField(1, body),
    );

    expect(TxWireHelper.txIdFromPropose(propose), txId);
    expect(
      TxWireHelper.txIdFromAck(TxWireHelper.encodeBytesField(1, txId)),
      txId,
    );
    expect(TxWireHelper.txIdFromAck(Uint8List(0)), isNull);
  });

  test('rejects a truncated field', () {
    expect(
      () => TxWireHelper.txIdFromAck(Uint8List.fromList(<int>[0x0a, 5, 1])),
      throwsFormatException,
    );
  });

  test('rejects an over-long varint and an overflowing length', () {
    final overlong = Uint8List.fromList(<int>[
      ...List<int>.filled(11, 0x80),
      0x01,
    ]);
    // field 1, length-delimited, length varint wraps negative (10 bytes).
    final negativeLength = Uint8List.fromList(<int>[
      0x0a,
      ...List<int>.filled(9, 0xff),
      0x01,
    ]);

    expect(() => TxWireHelper.txIdFromAck(overlong), throwsFormatException);
    expect(
      () => TxWireHelper.txIdFromAck(negativeLength),
      throwsFormatException,
    );
  });
}
