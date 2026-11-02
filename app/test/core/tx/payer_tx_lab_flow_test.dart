import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/l2cap_frame_codec.dart';
import 'package:meshpay/core/noise/noise_l2cap_session.dart';
import 'package:meshpay/core/tx/lab_frame_helper.dart';
import 'package:meshpay/core/tx/payer_tx_lab_flow.dart';
import 'package:meshpay/core/tx/tx_error_code.dart';
import 'package:meshpay/core/tx/tx_lab_message_helper.dart';
import 'package:meshpay/core/tx/tx_lab_state.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';

import '../../fakes/fake_native_lab_service.dart';
import '../../fakes/fake_noise_channel.dart';
import '../../fakes/fake_payer_tx_engine_service.dart';

/// Plays the receiver: decrypts each frame the flow sends and answers with
/// [onMessage], delivering the encrypted reply back to the session.
class _Peer {
  _Peer() {
    native.l2capResponder = (channelId, data) async {
      final plaintext = await channel.decrypt(L2capFrameCodec.decode(data));
      final frame = LabFrameHelper.decode(plaintext);
      final message = TxLabMessageHelper.decode(frame.payload);
      received.add(message.kind);
      final reply = await onMessage(message.kind, message.payload);
      final encrypted = await channel.encrypt(reply);
      Future<void>.microtask(
        () => session.deliver(L2capFrameCodec.encode(encrypted)),
      );
      return null;
    };
  }

  final FakeNativeLabService native = FakeNativeLabService();
  late FakeNoiseChannel channel;
  final List<TxLabMessageKind> received = <TxLabMessageKind>[];
  late NoiseL2capSession session;
  Future<Uint8List> Function(TxLabMessageKind, Uint8List) onMessage =
      (kind, payload) async => kind == TxLabMessageKind.hello
      ? TxLabMessageHelper.encode(
          TxLabMessageKind.helloReply,
          List<int>.filled(33, 3),
        )
      : TxLabMessageHelper.encode(
          TxLabMessageKind.ack,
          TxWireHelper.encodeBytesField(
            1,
            TxWireHelper.txIdFromPropose(payload)!,
          ),
        );
}

Future<NoiseL2capSession> _established(_Peer peer) async {
  final initiator = FakeNoiseChannel(isInitiator: true, binding: 1);
  final responder = peer.channel = FakeNoiseChannel(
    isInitiator: false,
    binding: 1,
  );
  await responder.readMessage(await initiator.writeMessage());
  await initiator.readMessage(await responder.writeMessage());
  await responder.readMessage(await initiator.writeMessage());
  return NoiseL2capSession(
    nativeService: peer.native,
    channelId: await peer.native.openL2cap(deviceId: 'AA', psm: 25),
    noise: initiator,
  );
}

void main() {
  late _Peer peer;
  late FakePayerTxEngineService engine;
  late PayerTxLabFlow flow;
  late List<TxLabState> states;

  setUp(() async {
    peer = _Peer();
    peer.session = await _established(peer);
    engine = FakePayerTxEngineService();
    states = <TxLabState>[];
    flow = PayerTxLabFlow(
      session: peer.session,
      engine: engine,
      onChanged: () {
        if (states.isEmpty || states.last != flow.state) {
          states.add(flow.state);
        }
      },
    );
  });

  test('success path goes locked, proposed, committed', () async {
    final linkLost = await flow.sendCheque();

    expect(linkLost, isFalse);
    expect(states, <TxLabState>[
      TxLabState.idle,
      TxLabState.locked,
      TxLabState.proposed,
      TxLabState.committed,
    ]);
    expect(flow.error, isNull);
    expect(flow.txIdHex, hasLength(8));
    expect(engine.calls, <String>['open', 'propose', 'onAck']);
    expect(peer.received, <TxLabMessageKind>[
      TxLabMessageKind.hello,
      TxLabMessageKind.propose,
    ]);
  });

  test(
    'a transport failure leaves the cheque locked and reports link loss',
    () async {
      final inner = peer.native.l2capResponder!;
      peer.native.l2capResponder = (channelId, data) async {
        if (peer.received.isNotEmpty) {
          throw StateError('link dropped');
        }
        return inner(channelId, data);
      };

      final linkLost = await flow.sendCheque();

      expect(linkLost, isTrue);
      expect(flow.state, TxLabState.locked);
      expect(flow.error, contains('Reenviar pendente'));
      expect(engine.proposeByCounter, hasLength(1));
    },
  );

  test('resendPending stops at the first failure', () async {
    for (final counter in <int>[1, 2]) {
      await engine.propose(
        spendBody: TxWireHelper.buildSpendBody(
          chequeId: Uint8List(16),
          payeeAccountId: Uint8List(33),
          amountUnits: 1,
          counter: counter,
          signedAtUnix: 1,
        ),
        counter: counter,
        recipientId: Uint8List(1),
      );
    }
    peer.onMessage = (kind, payload) async => kind == TxLabMessageKind.hello
        ? TxLabMessageHelper.encode(
            TxLabMessageKind.helloReply,
            List<int>.filled(33, 3),
          )
        : LabFrameHelper.encodeError(TxErrorCode.notAddressedToUs);

    final linkLost = await flow.resendPending();

    expect(linkLost, isFalse);
    expect(
      peer.received.where((kind) => kind == TxLabMessageKind.propose),
      hasLength(1),
    );
    expect(flow.state, TxLabState.failed);
    expect(flow.error, 'Receptor recusou: O cheque não é para esta conta.');
    expect(engine.committedCounters, isEmpty);
  });

  test(
    'cancel during in-flight work makes no engine call after close',
    () async {
      final gate = Completer<void>();
      final inner = peer.onMessage;
      peer.onMessage = (kind, payload) async {
        await gate.future;
        return inner(kind, payload);
      };

      final send = flow.sendCheque();
      await Future<void>.delayed(Duration.zero);
      await flow.cancel();
      expect(engine.calls, isEmpty);
      gate.complete();
      await send;

      expect(engine.calls, <String>['close']);
      expect(flow.state, TxLabState.idle);
      expect(flow.error, isNull);
    },
  );
}
