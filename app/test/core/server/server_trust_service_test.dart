import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/server/client/server_error_code.dart';
import 'package:meshpay/core/server/client/server_exception.dart';
import 'package:meshpay/core/server/responses/server_key_response.dart';
import 'package:meshpay/core/server/trust/server_trust_service.dart';
import 'package:meshpay/core/tx/issuer_config_helper.dart';
import 'package:meshpay/core/tx/receiver_clock_helper.dart';
import 'package:meshpay_core/meshpay_core.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../../fakes/fake_app_support_directory_service.dart';
import '../../fakes/fake_dtn_buffer_service.dart';
import '../../fakes/fake_external_tx_engines.dart';
import '../../fakes/fake_native_lab_service.dart';
import '../../fakes/fake_server_client.dart';

final Uint8List _root = Uint8List.fromList(List<int>.filled(32, 3));
final Uint8List _serverSnapshot = Uint8List.fromList(<int>[5, 6]);

void main() {
  late FakeServerClient client;
  late Directory dir;
  late ServerTrustService trust;

  setUp(() {
    client = FakeServerClient();
    dir = Directory.systemTemp.createTempSync('trust');
    trust = ServerTrustService(
      client,
      FakeAppSupportDirectoryService(dir.path),
      pinnedRoot: () async => _root,
    );
  });

  tearDown(() => dir.deleteSync(recursive: true));

  ServerTrustService withUrl(String url, {required bool isRelease}) =>
      ServerTrustService(
        client,
        FakeAppSupportDirectoryService(dir.path),
        configuredBaseUrl: url,
        isRelease: isRelease,
        pinnedRoot: () async => _root,
      );

  test(
    'issuer cert is returned when the server root matches the pinned one',
    () async {
      client.serverKey = ServerKeyResponse(
        rootPublicKey: _root,
        issuerCert: Uint8List.fromList(<int>[4, 2]),
      );

      expect(await trust.issuerCert(), <int>[4, 2]);
    },
  );

  test('a server root different from the pinned one is rejected', () async {
    client.serverKey = ServerKeyResponse(
      rootPublicKey: Uint8List.fromList(List<int>.filled(32, 1)),
      issuerCert: Uint8List.fromList(<int>[4, 2]),
    );

    await expectLater(trust.issuerCert(), throwsStateError);
    expect(File('${dir.path}/issuer_cert.bin').existsSync(), isFalse);
  });

  test('second call offline uses the stored cert', () async {
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: Uint8List.fromList(<int>[4, 2]),
    );
    await trust.issuerCert();
    client.trustError = const ServerException(ServerErrorCode.unreachable);

    expect(await trust.issuerCert(), <int>[4, 2]);
  });

  test('release without ISSUER_CERT_HEX takes the fetched cert', () async {
    final cert = await IssuerConfigHelper.cert(
      isRelease: true,
      configuredHex: '',
      fetch: () async => Uint8List.fromList(<int>[7]),
    );

    expect(cert, <int>[7]);
  });

  test('release without SERVER_BASE_URL fails closed', () {
    expect(() => withUrl('', isRelease: true).baseUrl, throwsStateError);
  });

  test('debug without SERVER_BASE_URL uses the lab loopback', () {
    expect(
      withUrl('', isRelease: false).baseUrl,
      ServerTrustService.labBaseUrl,
    );
  });

  test('offline refresh returns null instead of throwing', () async {
    client.trustError = const ServerException(ServerErrorCode.unreachable);

    expect(await trust.refreshSnapshot(), isNull);
  });

  test('a server error is treated as no fresh snapshot', () async {
    client.trustError = const ServerException(
      ServerErrorCode.malformedResponse,
    );

    expect(await trust.refreshSnapshot(), isNull);
  });

  group('prepare', () {
    late FakeDtnBufferService dtn;
    late FakeNativeLabService native;

    setUp(() {
      dtn = FakeDtnBufferService()..snapshot = Uint8List.fromList(<int>[9]);
      native = FakeNativeLabService()
        ..clockSamples.add(
          const ClockSample(elapsedRealtimeNanos: 42, bootCount: 7),
        );
    });

    Future<ClockSample> prepare(ReceiverTxEngine engine) =>
        ReceiverClockHelper.prepare(
          engine: engine,
          dtn: dtn,
          native: native,
          trust: trust,
        );

    test('fetched snapshot bytes are passed to installSnapshot', () async {
      client.snapshot = _serverSnapshot;
      final engine = FakeExternalReceiverEngine(<String>[]);

      await prepare(engine);

      expect(engine.installed?.first, _serverSnapshot);
    });

    test('offline falls back to the held snapshot without throwing', () async {
      client.trustError = const ServerException(ServerErrorCode.unreachable);
      final engine = FakeExternalReceiverEngine(<String>[]);

      await prepare(engine);

      expect(engine.installed?.first, <int>[9]);
    });

    for (final reason in <String>['older snapshot', 'bad signature']) {
      test('$reason from the server falls back to the held one', () async {
        client.snapshot = _serverSnapshot;
        final engine = _RefusingEngine(reason);

        await prepare(engine);

        expect(engine.calls, <String>['installSnapshot', 'installSnapshot']);
        expect(engine.installed?.first, <int>[9]);
      });
    }

    test('a refused held snapshot still propagates', () async {
      client.snapshot = _serverSnapshot;
      final engine = _RefusingEngine('bad signature', refuseAll: true);

      await expectLater(prepare(engine), throwsStateError);
    });
  });
}

class _RefusingEngine extends FakeExternalReceiverEngine {
  _RefusingEngine(this.reason, {this.refuseAll = false}) : super(<String>[]);

  final String reason;
  final bool refuseAll;

  @override
  Future<BigInt> installSnapshot({
    required List<int> snapshot,
    required BigInt monotonicNanos,
    BigInt? bootCount,
  }) async {
    if (refuseAll || listEquals(snapshot, _serverSnapshot)) {
      calls.add('installSnapshot');
      throw StateError(reason);
    }
    return super.installSnapshot(
      snapshot: snapshot,
      monotonicNanos: monotonicNanos,
      bootCount: bootCount,
    );
  }
}
