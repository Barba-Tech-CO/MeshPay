import 'dart:typed_data';

import 'package:meshpay/core/server/codec/proto_writer_helper.dart';
import 'package:meshpay/core/server/messages/cheque_stub.dart';
import 'package:meshpay/core/tx/lab_cheque_stub_helper.dart';
import 'package:meshpay/core/tx/payer_tx_engine_service.dart';
import 'package:meshpay/core/tx/tx_wire_helper.dart';
import 'package:protobuf/protobuf.dart';

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

  /// Unsealed stand-in for the Rust lab sealer (no native library in unit tests).
  @override
  Future<Uint8List> labStub({
    required Uint8List chequeId,
    required int maxAmountUnits,
    required int issuedAtUnix,
  }) async {
    final body = CodedBufferWriter();
    ProtoWriterHelper.bytes(body, 1, chequeId);
    ProtoWriterHelper.bytes(body, 5, await getPublicKey());
    ProtoWriterHelper.uint64(body, 6, maxAmountUnits);
    ProtoWriterHelper.uint64(
      body,
      7,
      issuedAtUnix + LabChequeStubHelper.expirySeconds,
    );
    return ChequeStub(
      body: body.toBuffer(),
      serverSignature: Uint8List(0),
    ).toBytes();
  }

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
