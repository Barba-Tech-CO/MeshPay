import 'dart:typed_data';

import 'package:meshpay/core/tx/payer_tx_engine_service.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';

/// Mirrors the Rust engine contract: propose is idempotent per counter,
/// `resumePending` returns the stored bytes, `onAck` commits.
class FakePayerTxEngineService implements PayerTxEngineService {
  final List<String> calls = <String>[];
  final Map<int, Uint8List> proposeByCounter = <int, Uint8List>{};
  final Set<int> committedCounters = <int>{};
  final List<Uint8List> acks = <Uint8List>[];
  bool get resumePendingEmpty =>
      proposeByCounter.keys.every(committedCounters.contains);
  int lastCounter = 0;
  Uint8List? openedPayeeKey;
  Object? proposeError;
  final List<Uint8List> stubs = <Uint8List>[];
  final List<Uint8List> spendBodies = <Uint8List>[];
  Object? ackError;

  @override
  Future<Uint8List> getPublicKey() async =>
      Uint8List.fromList(List<int>.filled(33, 2));

  @override
  Future<int> nextCounter() async => ++lastCounter;

  @override
  Future<void> close() async {
    calls.add('close');
  }

  @override
  Future<void> open({required Uint8List payeePublicKey}) async {
    calls.add('open');
    openedPayeeKey = payeePublicKey;
  }

  @override
  Future<Uint8List> propose({
    required Uint8List spendBody,
    required int counter,
    required Uint8List recipientId,
    required Uint8List stub,
  }) async {
    calls.add('propose');
    stubs.add(stub);
    spendBodies.add(spendBody);
    if (proposeError case final error?) {
      throw error;
    }
    return proposeByCounter.putIfAbsent(counter, () {
      final txId = Uint8List.fromList(List<int>.filled(32, counter % 251));
      final body = <int>[
        ...spendBody,
        ...TxWireHelper.encodeBytesField(4, txId),
      ];
      return TxWireHelper.encodeBytesField(
        2,
        TxWireHelper.encodeBytesField(1, body),
      );
    });
  }

  @override
  Future<void> onAck(Uint8List ack) async {
    calls.add('onAck');
    if (ackError case final error?) {
      throw error;
    }
    acks.add(ack);
    committedCounters.addAll(
      proposeByCounter.entries
          .where(
            (entry) =>
                TxWireHelper.txIdFromPropose(entry.value)!.first ==
                TxWireHelper.txIdFromAck(ack)!.first,
          )
          .map((entry) => entry.key),
    );
  }

  @override
  Future<List<Uint8List>> resumePending() async {
    calls.add('resumePending');
    return <Uint8List>[
      for (final entry in proposeByCounter.entries)
        if (!committedCounters.contains(entry.key)) entry.value,
    ];
  }
}
