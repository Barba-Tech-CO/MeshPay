import 'package:meshpay/features/lab/presentation/view_models/l2cap_receiver_lab_view_model.dart';
import 'package:meshpay_native/l2cap_closed_event.dart';
import 'package:meshpay_native/l2cap_data_event.dart';
import 'package:meshpay_native/l2cap_opened_event.dart';

import 'fake_native_lab_service.dart';
import 'fake_noise_service.dart';
import 'fake_permission_service.dart';
import 'fake_receiver_tx_engine_service.dart';

/// A receiver view model whose fake native layer can be wired to a payer's
/// fake native layer, so both ends talk through their real view models.
class L2capReceiverRig {
  L2capReceiverRig() {
    viewModel = L2capReceiverLabViewModel(
      native,
      FakePermissionService(),
      noise,
      tx,
    );
  }

  static const int serverChannelId = 900;

  final FakeNativeLabService native = FakeNativeLabService();
  final FakeNoiseService noise = FakeNoiseService();
  final FakeReceiverTxEngineService tx = FakeReceiverTxEngineService();
  late final L2capReceiverLabViewModel viewModel;

  Future<void> startAndAccept() async {
    await viewModel.start();
    viewModel.handleNativeEvent(
      const L2capOpenedEvent(
        channelId: serverChannelId,
        role: 'server',
        deviceId: 'PAYER',
      ),
    );
  }

  void linkTo(FakeNativeLabService payerNative) {
    payerNative.l2capResponder = (payerChannelId, data) async {
      final sentBefore = native.sentL2capPayloads.length;
      viewModel.handleNativeEvent(
        L2capDataEvent(channelId: serverChannelId, data: data),
      );
      await Future<void>.delayed(Duration.zero);
      if (native.closedChannels.contains(serverChannelId)) {
        payerNative.eventsController.add(
          L2capClosedEvent(channelId: payerChannelId),
        );
      }
      return native.sentL2capPayloads.length > sentBefore
          ? native.sentL2capPayloads.last
          : null;
    };
  }

  Future<void> dispose() async {
    viewModel.dispose();
    await Future<void>.delayed(Duration.zero);
  }
}
