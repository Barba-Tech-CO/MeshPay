import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/ble/l2cap_frame_codec.dart';
import 'package:meshpay/features/lab/presentation/view_models/l2cap_receiver_lab_view_model.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_permission_service.dart';

void main() {
  test('starts advertising and listening with an 8-byte session ID', () async {
    final native = FakeNativeLabService();
    final viewModel = L2capReceiverLabViewModel(
      native,
      FakePermissionService(),
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

    await viewModel.stop();
    expect(viewModel.status.name, 'closed');
    expect(native.stopAdvertisingCalls, 1);
    expect(native.stopListeningCalls, 1);
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  });

  test('echoes received bytes and counts only framed messages', () async {
    final native = FakeNativeLabService();
    final viewModel = L2capReceiverLabViewModel(
      native,
      FakePermissionService(),
    );
    final body = Uint8List.fromList(<int>[3, 4, 5]);
    final frame = L2capFrameCodec.encode(body);

    expect(await native.echoReceivedData(frame), frame);
    expect(viewModel.receivedMessageCount, 1);
    expect(
      await native.echoReceivedData(Uint8List.fromList(<int>[1, 2])),
      <int>[
        1,
        2,
      ],
    );
    expect(viewModel.receivedMessageCount, 1);
    expect(viewModel.error, contains('Quadro inválido'));

    viewModel.handleNativeEvent(
      const L2capOpenedEvent(
        channelId: 7,
        role: 'server',
        deviceId: 'AA:BB',
      ),
    );
    expect(viewModel.status.name, 'connected');
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
      );

      await viewModel.start();

      expect(viewModel.error, contains('permissões'));
      expect(native.startAdvertisingCalls, 0);
      expect(native.stopListeningCalls, 0);
      viewModel.dispose();
      await Future<void>.delayed(Duration.zero);
    },
  );
}
