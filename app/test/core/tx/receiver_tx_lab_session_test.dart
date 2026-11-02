import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/lab_frame_helper.dart';
import 'package:meshpay/core/tx/receiver_tx_lab_session.dart';
import 'package:meshpay/core/tx/tx_error_code.dart';
import 'package:meshpay/core/tx/tx_lab_message_helper.dart';
import 'package:meshpay/core/tx/tx_lab_state.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';

import '../../fakes/fake_receiver_tx_engine_service.dart';

void main() {
  late FakeReceiverTxEngineService engine;
  late ReceiverTxLabSession session;
  late List<Uint8List> sent;

  Future<void> send(Uint8List plaintext) async => sent.add(plaintext);

  Future<void> handle(TxLabMessageKind kind, List<int> payload) =>
      session.handle(
        LabFrameHelper.decode(TxLabMessageHelper.encode(kind, payload)).payload,
        send,
      );

  TxErrorCode lastError() => LabFrameHelper.errorCodeOf(
    LabFrameHelper.decode(sent.last).payload,
  );

  setUp(() {
    engine = FakeReceiverTxEngineService();
    session = ReceiverTxLabSession(engine, () {});
    sent = <Uint8List>[];
  });

  test('refuses a second HELLO while the engine is open', () async {
    final key = List<int>.filled(33, 2);

    await handle(TxLabMessageKind.hello, key);
    await handle(TxLabMessageKind.hello, key);

    expect(engine.calls, <String>['open']);
    expect(lastError(), TxErrorCode.helloRepeated);
    expect(session.state, TxLabState.failed);
  });

  test('refuses a PROPOSE before HELLO and a short key', () async {
    await handle(TxLabMessageKind.propose, <int>[1]);
    expect(lastError(), TxErrorCode.helloMissing);

    await handle(TxLabMessageKind.hello, List<int>.filled(10, 2));
    expect(lastError(), TxErrorCode.invalidKey);
    expect(engine.calls, isEmpty);
  });

  test('close releases the engine and allows a new HELLO', () async {
    final key = List<int>.filled(33, 2);
    await handle(TxLabMessageKind.hello, key);

    await session.close();
    await handle(TxLabMessageKind.hello, key);

    expect(engine.calls, <String>['open', 'close', 'open']);
    expect(session.error, isNull);
  });

  test(
    'a reply that fails to send still leaves the cheque committed',
    () async {
      final key = List<int>.filled(33, 2);
      await handle(TxLabMessageKind.hello, key);
      final propose = TxWireHelper.encodeBytesField(
        2,
        TxWireHelper.encodeBytesField(
          1,
          TxWireHelper.encodeBytesField(4, Uint8List(32)),
        ),
      );

      await expectLater(
        session.handle(
          LabFrameHelper.decode(
            TxLabMessageHelper.encode(TxLabMessageKind.propose, propose),
          ).payload,
          (_) async => throw StateError('link dropped'),
        ),
        throwsStateError,
      );

      expect(engine.proposes, hasLength(1));
      expect(session.state, TxLabState.committed);
    },
  );

  test(
    'close during an in-flight HELLO releases the handle it opened',
    () async {
      engine.openGate = Completer<void>();
      final hello = handle(TxLabMessageKind.hello, List<int>.filled(33, 2));
      await Future<void>.delayed(Duration.zero);

      await session.close();
      engine.openGate!.complete();
      await hello;

      expect(engine.calls, <String>['open', 'close', 'close']);
      await handle(TxLabMessageKind.propose, <int>[1]);
      expect(lastError(), TxErrorCode.helloMissing);
    },
  );
}
