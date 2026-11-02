import 'dart:async';
import 'dart:typed_data';

import 'package:meshpay/core/tx/receiver_tx_engine_service.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';

/// The same propose twice returns the same ack, like the Rust engine.
class FakeReceiverTxEngineService implements ReceiverTxEngineService {
  final List<String> calls = <String>[];
  final List<Uint8List> proposes = <Uint8List>[];
  Uint8List? openedPayerKey;
  Object? onProposeError;
  Object? closeError;
  Completer<void>? openGate;

  Uint8List publicKey = Uint8List.fromList(List<int>.filled(33, 3));

  @override
  Future<Uint8List> getPublicKey() async => publicKey;

  @override
  Future<void> close() async {
    calls.add('close');
    if (closeError case final error?) {
      throw error;
    }
  }

  @override
  Future<void> open({required Uint8List payerPublicKey}) async {
    calls.add('open');
    await openGate?.future;
    openedPayerKey = payerPublicKey;
  }

  @override
  Future<Uint8List> onPropose(Uint8List propose) async {
    calls.add('onPropose');
    if (onProposeError case final error?) {
      throw error;
    }
    proposes.add(propose);
    return TxWireHelper.encodeBytesField(
      1,
      TxWireHelper.txIdFromPropose(propose)!,
    );
  }
}
