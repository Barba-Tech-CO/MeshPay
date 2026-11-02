import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';

// Same literals as core/tests/tx_wire_golden.rs (prost decodes/encodes them).
const _spendBodyHex =
    '0a100102030405060708090a0b0c0d0e0f101208a0a1a2a3a4a5a6a71887ad4b2805'
    '3880e2cfaa06';
const _proposeHex =
    '12520a4a0a100102030405060708090a0b0c0d0e0f101208a0a1a2a3a4a5a6a71887ad4b28053880e2cfaa062220000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f1204eeeeeeee';
const _ackHex =
    '0a20000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f1204bbbbbbbb1a04cccccccc';

Uint8List _bytes(String hex) => Uint8List.fromList(<int>[
  for (var i = 0; i < hex.length; i += 2)
    int.parse(hex.substring(i, i + 2), radix: 16),
]);

String _hex(List<int> bytes) =>
    bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();

void main() {
  final txId = Uint8List.fromList(List<int>.generate(32, (i) => i));

  test('spend body encodes to the golden bytes prost decodes', () {
    final body = TxWireHelper.buildSpendBody(
      chequeId: Uint8List.fromList(List<int>.generate(16, (i) => i + 1)),
      payeeAccountId: Uint8List.fromList(
        List<int>.generate(8, (i) => 0xa0 + i),
      ),
      amountUnits: 1234567,
      counter: 5,
      signedAtUnix: 1700000000,
    );

    expect(_hex(body), _spendBodyHex);
  });

  test('reads tx_id from the golden propose and ack encoded by prost', () {
    expect(TxWireHelper.txIdFromPropose(_bytes(_proposeHex)), txId);
    expect(TxWireHelper.txIdFromAck(_bytes(_ackHex)), txId);
  });
}
