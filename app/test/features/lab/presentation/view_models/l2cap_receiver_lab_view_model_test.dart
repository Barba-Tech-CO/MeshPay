import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/l2cap_frame_codec.dart';
import 'package:meshpay/core/noise/noise_handshake_status.dart';
import 'package:meshpay/features/lab/presentation/helpers/lab_error_message_helper.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_receiver_lab_view_model.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_noise_channel.dart';
import '../../../../fakes/fake_noise_service.dart';
import '../../../../fakes/fake_permission_service.dart';
import '../../../../fakes/l2cap_receiver_rig.dart';

void main() {
  test('starts advertising and listening with an 8-byte session ID', () async {
    final native = FakeNativeLabService();
    final viewModel = L2capReceiverLabViewModel(
      native,
      FakePermissionService(),
      FakeNoiseService(),
    );

    await viewModel.start();

    expect(viewModel.sessionIdHex, hasLength(16));
    expect(native.advertisedSessionIdHex, viewModel.sessionIdHex);
    expect(viewModel.psm, 25);
    expect(viewModel.status.name, 'listening');
    expect(viewModel.qrPayload, contains('"psm":25'));
    expect(
      viewModel.qrPayload,
      contains('"session":"${viewModel.sessionIdHex}"'),
    );
    expect(viewModel.qrPayload, contains('"key":"${viewModel.keyHex}"'));
    expect(viewModel.keyHex, hasLength(64));

    await viewModel.stop();
    expect(viewModel.status.name, 'closed');
    expect(native.stopAdvertisingCalls, 1);
    expect(native.stopListeningCalls, 1);
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  });

  test('closes a server channel that opens without an active charge', () async {
    final native = FakeNativeLabService();
    final viewModel = L2capReceiverLabViewModel(
      native,
      FakePermissionService(),
      FakeNoiseService(),
    );

    viewModel.handleNativeEvent(
      const L2capOpenedEvent(
        channelId: 7,
        role: 'server',
        deviceId: 'AA:BB',
      ),
    );
    expect(viewModel.status.name, 'connected');
    await Future<void>.delayed(Duration.zero);
    expect(native.closedChannels, <int>[7]);

    viewModel.handleNativeEvent(const L2capClosedEvent(channelId: 7));
    expect(viewModel.status.name, 'closed');
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  });

  test('stops advertising when L2CAP listening cannot start', () async {
    final native = FakeNativeLabService()
      ..listeningError = StateError('listener unavailable');
    final viewModel = L2capReceiverLabViewModel(
      native,
      FakePermissionService(),
      FakeNoiseService(),
    );

    await viewModel.start();

    expect(viewModel.error, 'Ocorreu um erro inesperado. Tente novamente.');
    expect(native.startAdvertisingCalls, 1);
    expect(native.stopAdvertisingCalls, 1);
    expect(viewModel.status.name, 'closed');
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  });

  test(
    'does not start radio operations without Bluetooth permission',
    () async {
      final native = FakeNativeLabService();
      final viewModel = L2capReceiverLabViewModel(
        native,
        FakePermissionService()..granted = false,
        FakeNoiseService(),
      );

      await viewModel.start();

      expect(viewModel.error, contains('permissões'));
      expect(native.startAdvertisingCalls, 0);
      expect(native.stopListeningCalls, 0);
      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);
    },
  );

  test('generates a fresh static key on every start', () async {
    final noise = FakeNoiseService();
    final viewModel = L2capReceiverLabViewModel(
      FakeNativeLabService(),
      FakePermissionService(),
      noise,
    );
    final keys = <String>{};

    for (var start = 0; start < 3; start++) {
      await viewModel.start();
      keys.add(viewModel.keyHex!);
      await viewModel.stop();
    }

    expect(keys, hasLength(3));
    expect(noise.keypairCount, 3);
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  });

  Future<FakeNoiseChannel> sendMsg1(L2capReceiverRig rig) async {
    final initiator = FakeNoiseChannel(
      isInitiator: true,
      binding: rig.noise.responders.single.binding,
    );
    rig.viewModel.handleNativeEvent(
      L2capDataEvent(
        channelId: L2capReceiverRig.serverChannelId,
        data: L2capFrameCodec.encode(await initiator.writeMessage()),
      ),
    );
    await Future<void>.delayed(Duration.zero);
    return initiator;
  }

  Future<void> sendFrame(L2capReceiverRig rig, Uint8List body) async {
    rig.viewModel.handleNativeEvent(
      L2capDataEvent(
        channelId: L2capReceiverRig.serverChannelId,
        data: L2capFrameCodec.encode(body),
      ),
    );
    await Future<void>.delayed(Duration.zero);
  }

  test('acts as responder: reads msg1, writes msg2, reads msg3', () async {
    final rig = L2capReceiverRig();
    await rig.startAndAccept();
    final viewModel = rig.viewModel;
    final prologue = rig.noise.responderPrologues.single;

    expect(
      String.fromCharCodes(prologue),
      'meshpay/1|${viewModel.sessionIdHex}|${viewModel.psm}|${viewModel.keyHex}',
    );
    expect(viewModel.handshakeStatus, NoiseHandshakeStatus.inProgress);

    final initiator = await sendMsg1(rig);

    expect(rig.native.sentL2capPayloads, hasLength(1));
    expect(rig.native.sentL2capPayloads.single, hasLength(4 + 48));
    expect(viewModel.handshakeMessageSizes, <int>[48, 48]);
    expect(viewModel.handshakeStatus, NoiseHandshakeStatus.inProgress);
    expect(rig.native.stopListeningCalls, 0);

    await initiator.readMessage(
      L2capFrameCodec.decode(rig.native.sentL2capPayloads.single),
    );
    await sendFrame(rig, await initiator.writeMessage());

    expect(viewModel.handshakeMessageSizes, <int>[48, 48, 64]);
    expect(viewModel.handshakeStatus, NoiseHandshakeStatus.finished);
    expect(rig.native.sentL2capPayloads, hasLength(1));

    final plaintext = Uint8List.fromList(<int>[9, 8, 7]);
    await sendFrame(rig, await initiator.encrypt(plaintext));

    expect(rig.native.sentL2capPayloads, hasLength(2));
    final reply = L2capFrameCodec.decode(rig.native.sentL2capPayloads.last);
    expect(reply, isNot(plaintext));
    expect(await initiator.decrypt(reply), plaintext);
    expect(viewModel.receivedMessageCount, 1);
    await rig.dispose();
  });

  test(
    'wipes the charge and stops the radio once the handshake ends',
    () async {
      final rig = L2capReceiverRig();
      await rig.startAndAccept();
      final viewModel = rig.viewModel;
      expect(viewModel.qrPayload, isNotNull);
      final initiator = await sendMsg1(rig);
      await initiator.readMessage(
        L2capFrameCodec.decode(rig.native.sentL2capPayloads.single),
      );

      await sendFrame(rig, await initiator.writeMessage());

      expect(rig.native.stopListeningCalls, 1);
      expect(rig.native.stopAdvertisingCalls, 1);
      expect(viewModel.qrPayload, isNull);
      expect(viewModel.sessionIdHex, isNull);
      expect(viewModel.psm, isNull);
      expect(viewModel.keyHex, isNull);
      expect(
        rig.noise.responderPrivateKeys.single.every((b) => b == 0),
        isTrue,
      );
      expect(viewModel.status.name, 'connected');
      await rig.dispose();
    },
  );

  test('accepts only the first server channel per charge', () async {
    final rig = L2capReceiverRig();
    await rig.startAndAccept();

    rig.viewModel.handleNativeEvent(
      const L2capOpenedEvent(
        channelId: 901,
        role: 'server',
        deviceId: 'OTHER',
      ),
    );
    await Future<void>.delayed(Duration.zero);

    expect(rig.native.closedChannels, <int>[901]);
    expect(rig.noise.responders, hasLength(1));
    expect(rig.viewModel.status.name, 'connected');
    await rig.dispose();
  });

  test('a handshake that does not match fails and ends the charge', () async {
    final rig = L2capReceiverRig();
    await rig.startAndAccept();

    await sendFrame(rig, Uint8List(48));

    final viewModel = rig.viewModel;
    expect(viewModel.handshakeStatus, NoiseHandshakeStatus.failed);
    expect(viewModel.error, LabErrorMessageHelper.noiseHandshakeMessage);
    expect(rig.native.closedChannels, <int>[L2capReceiverRig.serverChannelId]);
    expect(rig.native.stopListeningCalls, 1);
    expect(rig.native.stopAdvertisingCalls, 1);
    expect(viewModel.qrPayload, isNull);
    expect(rig.noise.responderPrivateKeys.single.every((b) => b == 0), isTrue);
    expect(viewModel.status.name, 'closed');
    await rig.dispose();
  });

  test('an invalid frame length closes the channel', () async {
    final rig = L2capReceiverRig();
    await rig.startAndAccept();

    rig.viewModel.handleNativeEvent(
      L2capDataEvent(
        channelId: L2capReceiverRig.serverChannelId,
        data: Uint8List(4),
      ),
    );
    await Future<void>.delayed(Duration.zero);

    expect(rig.viewModel.error, contains('Quadro inválido'));
    expect(rig.native.closedChannels, <int>[L2capReceiverRig.serverChannelId]);
    expect(rig.native.stopListeningCalls, 1);
    await rig.dispose();
  });

  test(
    'a body above 512 bytes is rejected before the handshake ends',
    () async {
      final rig = L2capReceiverRig();
      await rig.startAndAccept();

      await sendFrame(rig, Uint8List(600));

      expect(rig.viewModel.error, contains('Quadro inválido'));
      expect(rig.native.closedChannels, <int>[
        L2capReceiverRig.serverChannelId,
      ]);
      expect(rig.native.sentL2capPayloads, isEmpty);
      await rig.dispose();
    },
  );

  test('a decrypt failure after the handshake closes the channel', () async {
    final rig = L2capReceiverRig();
    await rig.startAndAccept();
    final initiator = await sendMsg1(rig);
    await initiator.readMessage(
      L2capFrameCodec.decode(rig.native.sentL2capPayloads.single),
    );
    await sendFrame(rig, await initiator.writeMessage());

    await sendFrame(rig, Uint8List(40));

    expect(rig.viewModel.error, isNot(contains('handshake')));
    expect(rig.viewModel.error, isNotNull);
    expect(rig.native.closedChannels, <int>[L2capReceiverRig.serverChannelId]);
    expect(rig.viewModel.receivedMessageCount, 0);
    await rig.dispose();
  });

  test(
    'queued data is ignored when its channel closed before it ran',
    () async {
      final rig = L2capReceiverRig();
      await rig.startAndAccept();
      final initiator = FakeNoiseChannel(
        isInitiator: true,
        binding: rig.noise.responders.single.binding,
      );
      final msg1 = await initiator.writeMessage();

      rig.viewModel
        ..handleNativeEvent(
          L2capDataEvent(
            channelId: L2capReceiverRig.serverChannelId,
            data: L2capFrameCodec.encode(msg1),
          ),
        )
        ..handleNativeEvent(
          const L2capClosedEvent(channelId: L2capReceiverRig.serverChannelId),
        );
      await Future<void>.delayed(Duration.zero);

      expect(rig.native.sentL2capPayloads, isEmpty);
      expect(rig.viewModel.handshakeMessageSizes, isEmpty);
      await rig.dispose();
    },
  );

  test('stop wipes the static key and the QR state', () async {
    final rig = L2capReceiverRig();
    await rig.startAndAccept();

    await rig.viewModel.stop();

    expect(rig.viewModel.qrPayload, isNull);
    expect(rig.noise.responderPrivateKeys.single.every((b) => b == 0), isTrue);
    expect(rig.native.stopListeningCalls, 1);
    expect(rig.native.stopAdvertisingCalls, 1);
    expect(rig.native.closedChannels, <int>[L2capReceiverRig.serverChannelId]);
    await rig.dispose();
  });
}
