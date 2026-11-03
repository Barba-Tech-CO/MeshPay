import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/external_tx_signing_helper.dart';

import '../../fakes/fake_external_tx_engines.dart';
import '../../fakes/fake_payment_key_signer.dart';

void main() {
  late List<String> calls;
  late FakePaymentKeySigner signer;

  setUp(() {
    calls = <String>[];
    signer = FakePaymentKeySigner();
  });

  Future<Uint8List> propose(FakeExternalPayerEngine engine) =>
      ExternalTxSigningHelper.propose(
        engine: engine,
        signer: signer,
        alias: 'lab_payer',
        spendBody: Uint8List.fromList(<int>[1]),
        counter: BigInt.one,
        recipientId: Uint8List(65),
      );

  Future<Uint8List> acknowledge(FakeExternalReceiverEngine engine) =>
      ExternalTxSigningHelper.acknowledge(
        engine: engine,
        signer: signer,
        alias: 'lab_receiver',
        propose: Uint8List.fromList(<int>[1]),
      );

  test('payer prepares, signs the prepared message, then finishes', () async {
    final engine = FakeExternalPayerEngine(calls);

    final result = await propose(engine);

    expect(result, engine.proposeBytes);
    expect(calls, <String>['preparePropose', 'finishPropose']);
    expect(signer.calls, <String>['sign:lab_payer']);
    expect(signer.signedMessages.single, <int>[1, 2, 3]);
    expect(engine.finishedSignature, signer.signature);
  });

  test('payer skips signing when the propose already exists', () async {
    final stored = Uint8List.fromList(<int>[9, 9, 9]);
    final engine = FakeExternalPayerEngine(calls, existingPropose: stored);

    final result = await propose(engine);

    expect(result, stored);
    expect(calls, <String>['preparePropose']);
    expect(signer.calls, isEmpty);
  });

  test('payer does not finish when the Keystore refuses to sign', () async {
    signer.signError = StateError('key_unusable');
    final engine = FakeExternalPayerEngine(calls);

    await expectLater(propose(engine), throwsStateError);

    expect(calls, <String>['preparePropose']);
  });

  test('receiver stores, prepares the ack, signs, then finishes', () async {
    final engine = FakeExternalReceiverEngine(calls);

    final result = await acknowledge(engine);

    expect(result, engine.ackBytes);
    expect(calls, <String>['storePropose', 'prepareAck', 'finishAck']);
    expect(signer.calls, <String>['sign:lab_receiver']);
    expect(signer.signedMessages.single, <int>[5, 6]);
    expect(engine.finishedSignature, signer.signature);
  });

  test('receiver skips signing when the ack already exists', () async {
    final stored = Uint8List.fromList(<int>[3, 3]);
    final engine = FakeExternalReceiverEngine(calls, existingAck: stored);

    final result = await acknowledge(engine);

    expect(result, stored);
    expect(calls, <String>['storePropose', 'prepareAck']);
    expect(signer.calls, isEmpty);
  });
}
