import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/dtn_gossip_session.dart';
import 'package:meshpay/core/dtn/dtn_message_helper.dart';
import 'package:meshpay/core/tx/lab_frame_helper.dart';
import 'package:meshpay/core/tx/tx_error_code.dart';

import '../../fakes/fake_dtn_buffer_service.dart';

void main() {
  late FakeDtnBufferService payerBuffer;
  late FakeDtnBufferService receiverBuffer;
  late DtnGossipSession payer;
  late DtnGossipSession receiver;
  var failExchange = false;

  Future<Uint8List> exchange(Uint8List plaintext) async {
    if (failExchange) {
      throw StateError('link down');
    }
    late Uint8List reply;
    await receiver.handle(
      LabFrameHelper.decode(plaintext).payload,
      (frame) async => reply = frame,
    );
    return reply;
  }

  setUp(() {
    failExchange = false;
    payerBuffer = FakeDtnBufferService();
    receiverBuffer = FakeDtnBufferService();
    payer = DtnGossipSession(payerBuffer, () {})..startConnection();
    receiver = DtnGossipSession(receiverBuffer, () {})..startConnection();
  });

  test('both sides converge and report their counts', () async {
    await payerBuffer.addTestEnvelope(Uint8List(8));
    await payerBuffer.addTestEnvelope(Uint8List(8));
    await payerBuffer.addTestEnvelope(Uint8List(8));
    await receiverBuffer.addTestEnvelope(Uint8List(8));
    await receiverBuffer.addTestEnvelope(Uint8List(8));

    final linkLost = await payer.sync(exchange);

    expect(linkLost, isFalse);
    expect(payer.error, isNull);
    expect(payerBuffer.held.keys.toSet(), receiverBuffer.held.keys.toSet());
    expect(payer.stats.held, 5);
    expect(payer.stats.sent, 3);
    expect(payer.stats.received, 2);
    expect(receiver.stats.held, 5);
    expect(receiver.stats.sent, 2);
    expect(receiver.stats.received, 3);
    expect(payer.stats.duplicates + receiver.stats.duplicates, 0);
  });

  test('envelopes the peer already holds are not sent again', () async {
    await payerBuffer.addTestEnvelope(Uint8List(8));
    await payer.sync(exchange);
    payer.startConnection();
    receiver.startConnection();

    await payer.sync(exchange);

    expect(payer.stats.sent, 1);
    expect(receiver.stats.sent, 0);
    expect(payer.stats.duplicates, 0);
    expect(receiver.stats.duplicates, 0);
  });

  test('an envelope that arrives anyway counts as a duplicate', () async {
    await payerBuffer.addTestEnvelope(Uint8List(8));
    await payer.sync(exchange);
    payer.startConnection();
    receiver.startConnection();
    payerBuffer.ignorePeerSummary = true;

    await payer.sync(exchange);

    expect(payer.stats.sent, 2);
    expect(receiver.stats.duplicates, 1);
    expect(receiver.stats.received, 1);
    expect(receiverBuffer.held, hasLength(1));
  });

  test('the session budget error is shown in Portuguese', () async {
    receiverBuffer.sessionBudget = 2;
    for (var count = 0; count < 3; count++) {
      await payerBuffer.addTestEnvelope(Uint8List(8));
    }

    await payer.sync(exchange);

    expect(receiver.stats.received, 2);
    expect(receiver.stats.rejected, 1);
    expect(
      receiver.error,
      'O limite de 20 envelopes desta conexão foi atingido.',
    );
    expect(receiverBuffer.held, hasLength(2));
  });

  test('a newer snapshot from the peer sets the last version', () async {
    receiverBuffer
      ..snapshot = Uint8List.fromList(<int>[3, 1])
      ..snapshotVersion = 3;

    await payer.sync(exchange);

    expect(payer.stats.snapshotVersion, 3);
    expect(payerBuffer.snapshotVersion, 3);
    expect(receiver.stats.snapshotVersion, isNull);
  });

  test('a lost link is reported so the caller can tear down', () async {
    failExchange = true;

    final linkLost = await payer.sync(exchange);

    expect(linkLost, isTrue);
    expect(payer.error, contains('link foi interrompido'));
  });

  test('the receiver refuses an envelope before the summary', () async {
    late Uint8List reply;
    await receiver.handle(
      Uint8List.fromList(<int>[1, 9]),
      (frame) async => reply = frame,
    );

    expect(LabFrameHelper.decode(reply).type, LabFrameType.error);
    expect(receiver.error, isNotNull);
  });

  test('creates a test envelope in the local buffer', () async {
    await payer.createTestEnvelope();

    expect(payerBuffer.held, hasLength(1));
    expect(payer.stats.held, 1);
  });

  test('a refused peer snapshot is counted on both sides', () async {
    final snapshot = Uint8List.fromList(<int>[3, 1]);
    payerBuffer
      ..snapshot = snapshot
      ..snapshotVersion = 3;
    receiverBuffer
      ..snapshot = snapshot
      ..snapshotVersion = 3;

    await payer.sync(exchange);

    expect(payer.stats.snapshotsRefused, 1);
    expect(receiver.stats.snapshotsRefused, 1);
    expect(payer.stats.snapshotVersion, isNull);
  });

  test('a second sync on the same connection does not send', () async {
    await payer.sync(exchange);
    var sent = 0;

    await payer.sync((plaintext) {
      sent++;
      return exchange(plaintext);
    });

    expect(sent, 0);
    expect(payer.error, contains('já sincronizou'));
  });

  test('the receiver rejects a second summary on one connection', () async {
    final summary = DtnMessageHelper.encode(
      DtnMessageKind.summary,
      const <int>[],
    );
    Uint8List? reply;
    Future<void> send(Uint8List frame) async => reply = frame;

    await receiver.handle(summary.sublist(1), send);
    expect(LabFrameHelper.decode(reply!).type, LabFrameType.dtn);
    await receiver.handle(summary.sublist(1), send);

    final frame = LabFrameHelper.decode(reply!);
    expect(frame.type, LabFrameType.error);
    expect(
      LabFrameHelper.errorCodeOf(frame.payload),
      TxErrorCode.dtnSummaryRepeated,
    );
  });

  test('the receiver cuts off a peer looping empty envelopes', () async {
    Uint8List? reply;
    Future<void> send(Uint8List frame) async => reply = frame;
    await receiver.handle(
      DtnMessageHelper.encode(DtnMessageKind.summary, const <int>[]).sublist(1),
      send,
    );
    final empty = DtnMessageHelper.encode(
      DtnMessageKind.envelope,
      const <int>[],
    ).sublist(1);

    for (var round = 0; round < DtnGossipSession.maximumRounds; round++) {
      await receiver.handle(empty, send);
      expect(LabFrameHelper.decode(reply!).type, LabFrameType.dtn);
    }
    await receiver.handle(empty, send);

    final frame = LabFrameHelper.decode(reply!);
    expect(frame.type, LabFrameType.error);
    expect(
      LabFrameHelper.errorCodeOf(frame.payload),
      TxErrorCode.dtnRoundLimit,
    );
  });

  test('a peer that never drains cuts the sync and is counted', () async {
    var counter = 0;
    Future<Uint8List> neverDrains(Uint8List plaintext) async {
      final kind = DtnMessageHelper.decode(
        LabFrameHelper.decode(plaintext).payload,
      ).kind;
      final body = kind == DtnMessageKind.envelope
          ? <int>[counter++, ...List<int>.filled(40, 7)]
          : const <int>[];
      return DtnMessageHelper.encode(kind, body);
    }

    await payer.sync(neverDrains);

    expect(payer.stats.cutSyncs, 1);
    expect(payer.error, contains('limite de rodadas'));
    expect(counter, DtnGossipSession.maximumRounds);
  });

  test('outbound is capped at 20 envelopes', () async {
    for (var count = 0; count < 25; count++) {
      await payerBuffer.addTestEnvelope(Uint8List(8));
    }

    await payer.sync(exchange);

    expect(payer.stats.sent, 20);
    expect(receiverBuffer.held, hasLength(20));
  });
}
