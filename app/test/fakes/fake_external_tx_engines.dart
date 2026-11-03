import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

/// Records the call order; unimplemented members of the opaque Rust handle
/// are never reached by the signing helper.
class FakeExternalPayerEngine implements PayerTxEngine {
  FakeExternalPayerEngine(this.calls, {this.existingPropose});

  final List<String> calls;
  final Uint8List? existingPropose;
  final Uint8List proposeBytes = Uint8List.fromList(<int>[7, 7]);
  Uint8List? finishedSignature;

  @override
  Future<PreparedPropose> preparePropose({
    required List<int> spendBody,
    required BigInt counter,
    required List<int> recipientId,
  }) async {
    calls.add('preparePropose');
    return PreparedPropose(
      message: Uint8List.fromList(<int>[1, 2, 3]),
      body: Uint8List.fromList(spendBody),
      counter: counter,
      existingPropose: existingPropose,
    );
  }

  @override
  Future<Uint8List> finishPropose({
    required PreparedPropose prepared,
    required List<int> signature,
  }) async {
    calls.add('finishPropose');
    finishedSignature = Uint8List.fromList(signature);
    return proposeBytes;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class FakeExternalReceiverEngine implements ReceiverTxEngine {
  FakeExternalReceiverEngine(this.calls, {this.existingAck});

  final List<String> calls;
  final Uint8List? existingAck;
  final Uint8List txId = Uint8List.fromList(<int>[4, 4]);
  final Uint8List ackBytes = Uint8List.fromList(<int>[8, 8]);
  Uint8List? finishedSignature;

  @override
  Future<Uint8List> storePropose({required List<int> propose}) async {
    calls.add('storePropose');
    return txId;
  }

  @override
  Future<PreparedAck> prepareAck({required List<int> txId}) async {
    calls.add('prepareAck');
    return PreparedAck(
      txId: this.txId,
      message: Uint8List.fromList(<int>[5, 6]),
      existingAck: existingAck,
    );
  }

  @override
  Future<Uint8List> finishAck({
    required List<int> txId,
    required List<int> signature,
  }) async {
    calls.add('finishAck');
    finishedSignature = Uint8List.fromList(signature);
    return ackBytes;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
