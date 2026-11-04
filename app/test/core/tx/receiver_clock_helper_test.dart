import 'package:flutter/foundation.dart';
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

  test('a refused held snapshot is logged and does not fail prepare', () async {
    dtn.snapshot = Uint8List.fromList(<int>[9]);
    engine.installError = StateError('snapshot_rejected');
    final messages = <String>[];
    final previousDebugPrint = debugPrint;
    debugPrint = (String? message, {int? wrapWidth}) {
      if (message != null) messages.add(message);
    };
    addTearDown(() => debugPrint = previousDebugPrint);

    final sample = await ReceiverClockHelper.prepare(
      engine: engine,
      dtn: dtn,
      native: native,
    );

    expect(sample.elapsedRealtimeNanos, 42);
    expect(engine.calls, <String>['installSnapshot']);
    expect(engine.installed, isNull);
    expect(messages, <String>['server snapshot refused: StateError']);
  });

  test(
    'an out-of-window held snapshot is logged before core refuses the propose',
    () async {
      dtn.snapshot = Uint8List.fromList(<int>[9]);
      final engine = _NoClockAnchorEngine(calls)
        ..installError = StateError('snapshot issued_at outside cert window');
      final messages = <String>[];
      final previousDebugPrint = debugPrint;
      debugPrint = (String? message, {int? wrapWidth}) {
        if (message != null) messages.add(message);
      };
      addTearDown(() => debugPrint = previousDebugPrint);

      final sample = await ReceiverClockHelper.prepare(
        engine: engine,
        dtn: dtn,
        native: native,
      );

      await expectLater(
        ExternalTxSigningHelper.acknowledge(
          engine: engine,
          signer: FakePaymentKeySigner(),
          alias: 'lab_receiver',
          propose: Uint8List.fromList(<int>[1]),
          clock: sample,
        ),
        throwsA(
          isA<StateError>().having(
            (error) => error.message,
            'message',
            'no revocation snapshot',
          ),
        ),
      );

      expect(engine.calls, <String>['installSnapshot', 'storePropose']);
      expect(engine.installed, isNull);
      expect(messages, <String>['server snapshot refused: StateError']);
    },
  );

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

class _NoClockAnchorEngine extends FakeExternalReceiverEngine {
  _NoClockAnchorEngine(super.calls);

  @override
  Future<Uint8List> storePropose({
    required List<int> propose,
    required BigInt monotonicNanos,
    BigInt? bootCount,
  }) async {
    calls.add('storePropose');
    throw StateError('no revocation snapshot');
  }
}
