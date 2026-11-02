import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/l2cap_frame_codec.dart';
import 'package:meshpay/core/noise/noise_handshake_status.dart';
import 'package:meshpay/core/noise/noise_l2cap_session.dart';

import '../../fakes/fake_native_lab_service.dart';
import '../../fakes/fake_noise_channel.dart';

Uint8List frame(List<int> body) =>
    L2capFrameCodec.encode(Uint8List.fromList(body));

NoiseL2capSession newSession(
  FakeNativeLabService native, {
  FakeNoiseChannel? noise,
  Duration timeout = const Duration(seconds: 5),
}) => NoiseL2capSession(
  nativeService: native,
  channelId: 77,
  noise: noise ?? FakeNoiseChannel(isInitiator: true, binding: 1),
  frameTimeout: timeout,
);

Future<FakeNoiseChannel> finishedInitiator() async {
  final initiator = FakeNoiseChannel(isInitiator: true, binding: 1);
  final responder = FakeNoiseChannel(isInitiator: false, binding: 1);
  await responder.readMessage(await initiator.writeMessage());
  await initiator.readMessage(await responder.writeMessage());
  await responder.readMessage(await initiator.writeMessage());
  return initiator;
}

void main() {
  test(
    'bytes that arrive before an exchange are buffered, not dropped',
    () async {
      final native = FakeNativeLabService();
      final session = newSession(native);
      final reply = frame(<int>[1, 2]);

      session
        ..deliver(reply.sublist(0, 3))
        ..deliver(reply.sublist(3));
      final response = await session.exchange(Uint8List.fromList(<int>[9]));

      expect(response, <int>[1, 2]);
      expect(native.sentL2capPayloads.single, frame(<int>[9]));
    },
  );

  test('several frames in one chunk serve successive exchanges', () async {
    final session = newSession(FakeNativeLabService());

    session.deliver(
      Uint8List.fromList(<int>[
        ...frame(<int>[1]),
        ...frame(<int>[2]),
      ]),
    );

    expect(await session.exchange(Uint8List(1)), <int>[1]);
    expect(await session.exchange(Uint8List(1)), <int>[2]);
  });

  test('an exchange without a reply times out', () async {
    final session = newSession(
      FakeNativeLabService(),
      timeout: const Duration(milliseconds: 20),
    );

    await expectLater(
      session.exchange(Uint8List(1)),
      throwsA(isA<TimeoutException>()),
    );
  });

  test(
    'frames above 512 bytes are rejected until the handshake ends',
    () async {
      final big = frame(List<int>.filled(600, 1));

      expect(
        () => newSession(FakeNativeLabService()).deliver(big),
        throwsFormatException,
      );

      final established = newSession(
        FakeNativeLabService(),
        noise: await finishedInitiator(),
      )..deliver(big);
      expect(await established.exchange(Uint8List(1)), hasLength(600));
    },
  );

  test(
    'close fails the pending exchange and closes the channel once',
    () async {
      final native = FakeNativeLabService();
      final session = newSession(native);
      final pending = session.exchange(Uint8List(1));
      final expectation = expectLater(pending, throwsStateError);

      await session.close();
      await session.close();

      await expectation;
      expect(native.closedChannels, <int>[77]);
      expect(session.isClosed, isTrue);
      expect(session.status, NoiseHandshakeStatus.failed);
    },
  );
}
