import 'dart:typed_data';

import '../../../../core/dtn/dtn_buffer_service.dart';
import '../../../../core/dtn/dtn_gossip_session.dart';
import '../../../../core/noise/noise_l2cap_session.dart';
import '../../../../core/tx/lab_frame_helper.dart';
import '../../../../core/tx/receiver_tx_engine_service.dart';
import '../../../../core/tx/receiver_tx_lab_session.dart';

/// Dispatches the decrypted application frames of the receiver page to the
/// echo, transaction and DTN handlers.
class L2capReceiverFrameHandler {
  L2capReceiverFrameHandler(
    ReceiverTxEngineService receiverTx,
    DtnBufferService dtn,
    void Function() onChanged,
  ) : tx = ReceiverTxLabSession(receiverTx, onChanged),
      gossip = DtnGossipSession(dtn, onChanged);

  final ReceiverTxLabSession tx;
  final DtnGossipSession gossip;
  int receivedMessageCount = 0;

  /// Handles one encrypted frame body; returns the handler's error text to
  /// show, or null. Throws on an undecryptable or invalid frame.
  Future<String?> handle(NoiseL2capSession session, Uint8List body) async {
    final plaintext = await session.decrypt(body);
    final frame = LabFrameHelper.decode(plaintext);
    switch (frame.type) {
      case LabFrameType.echo:
        await session.sendEncrypted(plaintext);
        receivedMessageCount++;
        return null;
      case LabFrameType.tx:
        await tx.handle(frame.payload, session.sendEncrypted);
        return tx.error;
      case LabFrameType.dtn:
        await gossip.handle(frame.payload, session.sendEncrypted);
        return gossip.error;
      case LabFrameType.error:
        throw const FormatException('Quadro de erro inesperado.');
    }
  }
}
