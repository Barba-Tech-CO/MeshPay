import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/external_tx_signing_helper.dart';
import 'package:meshpay/core/tx/receiver_clock_helper.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../../fakes/fake_dtn_buffer_service.dart';
import '../../fakes/fake_external_tx_engines.dart';
import '../../fakes/fake_native_lab_service.dart';
import '../../fakes/fake_payment_key_signer.dart';

void main() {
  late FakeDtnBufferService dtn;
  late FakeNativeLabService native;
  late FakeExternalReceiverEngine engine;
  late List<String> calls;

  setUp(() {
    dtn = FakeDtnBufferService();
    native = FakeNativeLabService()
      ..clockSamples.add(
        const ClockSample(elapsedRealtimeNanos: 42, bootCount: 7),
      );
    calls = <String>[];
    engine = FakeExternalReceiverEngine(calls);
  });

  test('installs the held snapshot with the same clock reading', () async {
    dtn.snapshot = Uint8List.fromList(<int>[9]);

    final sample = await ReceiverClockHelper.prepare(
      engine: engine,
      dtn: dtn,
      native: native,
    );

    expect(sample.elapsedRealtimeNanos, 42);
    expect(engine.calls, <String>['installSnapshot']);
    expect(engine.installed, <Object?>[
      <int>[9],
      BigInt.from(42),
      BigInt.from(7),
    ]);
  });

  test('without a snapshot nothing is installed (engine refuses)', () async {
    await ReceiverClockHelper.prepare(engine: engine, dtn: dtn, native: native);

    expect(engine.calls, isEmpty);
  });

  test('a null boot count is installed as null', () async {
    native.clockSamples
      ..clear()
      ..add(const ClockSample(elapsedRealtimeNanos: 55, bootCount: null));
    dtn.snapshot = Uint8List.fromList(<int>[9]);

    await ReceiverClockHelper.prepare(engine: engine, dtn: dtn, native: native);

    expect(engine.installed, <Object?>[
      <int>[9],
      BigInt.from(55),
      null,
    ]);
  });

  test('a failing installSnapshot escapes prepare as the raw error', () async {
    dtn.snapshot = Uint8List.fromList(<int>[9]);
    engine.installError = StateError('snapshot_rejected');

    await expectLater(
      ReceiverClockHelper.prepare(engine: engine, dtn: dtn, native: native),
      throwsA(
        isA<StateError>().having(
          (e) => e.message,
          'message',
          'snapshot_rejected',
        ),
      ),
    );
  });

  test('the snapshot is installed before the propose is stored with the '
      'same clock reading', () async {
    dtn.snapshot = Uint8List.fromList(<int>[9]);
    final signer = FakePaymentKeySigner();

    // Same composition as MeshpayCoreExternalReceiverTxEngineService.onPropose.
    await ExternalTxSigningHelper.acknowledge(
      engine: engine,
      signer: signer,
      alias: 'lab_receiver',
      propose: Uint8List.fromList(<int>[1]),
      clock: await ReceiverClockHelper.prepare(
        engine: engine,
        dtn: dtn,
        native: native,
      ),
    );

    expect(calls, <String>[
      'installSnapshot',
      'storePropose',
      'prepareAck',
      'finishAck',
    ]);
    expect(engine.installed?.sublist(1), engine.stored?.sublist(1));
    expect(engine.stored?.sublist(1), <Object?>[
      BigInt.from(42),
      BigInt.from(7),
    ]);
  });
}
