import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/payer_dtn_lab_flow.dart';

import '../../fakes/fake_dtn_buffer_service.dart';

void main() {
  test('a call made while another runs is ignored', () async {
    final buffer = FakeDtnBufferService();
    final flow = PayerDtnLabFlow(buffer, () {});
    final release = Completer<void>();
    final first = flow.sync((_) async {
      await release.future;
      throw StateError('link down');
    });
    await Future<void>.delayed(Duration.zero);

    final second = await flow.sync((_) async => Uint8List(0));
    await flow.createTestEnvelope();
    release.complete();
    final firstLinkLost = await first;

    expect(second, isFalse);
    expect(buffer.held, isEmpty);
    expect(firstLinkLost, isTrue);
  });

  test('is usable again after a run that failed', () async {
    final buffer = FakeDtnBufferService()..addError = StateError('disk full');
    final flow = PayerDtnLabFlow(buffer, () {});

    await flow.createTestEnvelope();
    expect(flow.error, isNotNull);
    buffer.addError = null;
    await flow.createTestEnvelope();

    expect(buffer.held, hasLength(1));
    expect(flow.error, isNull);
  });
}
