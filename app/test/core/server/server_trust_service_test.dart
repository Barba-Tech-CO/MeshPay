import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/server/client/server_error_code.dart';
import 'package:meshpay/core/server/client/server_exception.dart';
import 'package:meshpay/core/server/codec/proto_writer_helper.dart';
import 'package:meshpay/core/server/responses/server_key_response.dart';
import 'package:meshpay/core/server/trust/server_trust_service.dart';
import 'package:meshpay/core/tx/issuer_config_helper.dart';
import 'package:meshpay/core/tx/receiver_clock_helper.dart';
import 'package:meshpay_core/meshpay_core.dart';
import 'package:meshpay_native/clock_sample.dart';
import 'package:protobuf/protobuf.dart';

import '../../fakes/fake_app_support_directory_service.dart';
import '../../fakes/fake_dtn_buffer_service.dart';
import '../../fakes/fake_external_tx_engines.dart';
import '../../fakes/fake_native_lab_service.dart';
import '../../fakes/fake_server_client.dart';

final Uint8List _root = Uint8List.fromList(List<int>.filled(32, 3));
final Uint8List _serverSnapshot = Uint8List.fromList(<int>[5, 6]);
final Uint8List _issuerKeyId = Uint8List.fromList(<int>[0xab, 0xcd]);
const int _nowUnix = 1700000000;
const int _daySeconds = 24 * 3600;
final DateTime _now = DateTime.fromMillisecondsSinceEpoch(_nowUnix * 1000);

Uint8List _certBytes(int notAfterUnix) {
  final body = CodedBufferWriter();
  ProtoWriterHelper.bytes(body, 1, _issuerKeyId);
  ProtoWriterHelper.uint64(body, 5, notAfterUnix);
  final cert = CodedBufferWriter();
  ProtoWriterHelper.bytes(cert, 1, body.toBuffer());
  return Uint8List.fromList(cert.toBuffer());
}

File _certFile(Directory dir) => File('${dir.path}/issuer_cert.bin');

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
      clock: () => _now,
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
        clock: () => _now,
      );

  test(
    'issuer cert is returned when the server root matches the pinned one',
    () async {
      final cert = _certBytes(_nowUnix + 20 * _daySeconds);
      client.serverKey = ServerKeyResponse(
        rootPublicKey: _root,
        issuerCert: cert,
      );

      expect(await trust.issuerCert(), cert);
    },
  );

  test('a server root different from the pinned one is rejected', () async {
    client.serverKey = ServerKeyResponse(
      rootPublicKey: Uint8List.fromList(List<int>.filled(32, 1)),
      issuerCert: _certBytes(_nowUnix + 20 * _daySeconds),
    );

    await expectLater(trust.issuerCert(), throwsStateError);
    expect(_certFile(dir).existsSync(), isFalse);
  });

  test('second call offline uses the stored cert', () async {
    final cert = _certBytes(_nowUnix + 20 * _daySeconds);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: cert,
    );
    await trust.issuerCert();
    client.trustError = const ServerException(ServerErrorCode.unreachable);

    expect(await trust.issuerCert(), cert);
    expect(client.calls.where((call) => call == 'serverKey'), hasLength(1));
  });

  test('a fresh stored cert is used without a fetch', () async {
    final cert = _certBytes(_nowUnix + 20 * _daySeconds);
    _certFile(dir).writeAsBytesSync(cert);

    expect(await trust.issuerCert(), cert);
    expect(client.calls, isEmpty);
  });

  test('a stored cert near expiry is refetched and replaced', () async {
    _certFile(dir).writeAsBytesSync(_certBytes(_nowUnix + 3 * _daySeconds));
    final fresh = _certBytes(_nowUnix + 30 * _daySeconds);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: fresh,
    );

    expect(await trust.issuerCert(), fresh);
    expect(_certFile(dir).readAsBytesSync(), fresh);
  });

  test('an older refetched cert keeps the stored file', () async {
    final stored = _certBytes(_nowUnix + 3 * _daySeconds);
    _certFile(dir).writeAsBytesSync(stored);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: _certBytes(_nowUnix + _daySeconds),
    );

    expect(await trust.issuerCert(), stored);
    expect(_certFile(dir).readAsBytesSync(), stored);
  });

  test('a refetch that does not extend validity is not repeated', () async {
    final stored = _certBytes(_nowUnix + 3 * _daySeconds);
    _certFile(dir).writeAsBytesSync(stored);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: stored,
    );

    expect(await trust.issuerCert(), stored);
    expect(await trust.issuerCert(), stored);
    expect(client.calls.where((call) => call == 'serverKey'), hasLength(1));
  });

  test(
    'an expired stored cert is refetched after a no-extension refetch',
    () async {
      final stored = _certBytes(_nowUnix + 3 * _daySeconds);
      _certFile(dir).writeAsBytesSync(stored);
      client.serverKey = ServerKeyResponse(
        rootPublicKey: _root,
        issuerCert: stored,
      );
      var nowUnix = _nowUnix;
      final advancing = ServerTrustService(
        client,
        FakeAppSupportDirectoryService(dir.path),
        pinnedRoot: () async => _root,
        clock: () => DateTime.fromMillisecondsSinceEpoch(nowUnix * 1000),
      );

      expect(await advancing.issuerCert(), stored);
      expect(client.calls.where((call) => call == 'serverKey'), hasLength(1));

      nowUnix = _nowUnix + 4 * _daySeconds;
      final fresh = _certBytes(_nowUnix + 30 * _daySeconds);
      client.serverKey = ServerKeyResponse(
        rootPublicKey: _root,
        issuerCert: fresh,
      );

      expect(await advancing.issuerCert(), fresh);
      expect(client.calls.where((call) => call == 'serverKey'), hasLength(2));
    },
  );

  test('a failed rename removes the temporary file', () async {
    Directory(_certFile(dir).path).createSync();
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: _certBytes(_nowUnix + 20 * _daySeconds),
    );

    await expectLater(trust.issuerCert(), throwsA(isA<FileSystemException>()));
    final temporaryFiles = dir.listSync().whereType<File>().where(
      (entry) => entry.path.contains('issuer_cert.bin.tmp'),
    );
    expect(temporaryFiles, isEmpty);
  });

  test('an expired stored cert is refetched when the server answers', () async {
    _certFile(dir).writeAsBytesSync(_certBytes(_nowUnix - _daySeconds));
    final fresh = _certBytes(_nowUnix + 30 * _daySeconds);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: fresh,
    );

    expect(await trust.issuerCert(), fresh);
    expect(_certFile(dir).readAsBytesSync(), fresh);
  });

  test('an undecodable stored cert is refetched and replaced', () async {
    _certFile(dir).writeAsBytesSync(Uint8List.fromList(<int>[4, 2]));
    final fresh = _certBytes(_nowUnix + 30 * _daySeconds);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: _root,
      issuerCert: fresh,
    );

    expect(await trust.issuerCert(), fresh);
    expect(_certFile(dir).readAsBytesSync(), fresh);
  });

  test('an expired stored cert with the server offline throws', () async {
    _certFile(dir).writeAsBytesSync(_certBytes(_nowUnix - _daySeconds));
    client.trustError = const ServerException(ServerErrorCode.unreachable);

    await expectLater(
      trust.issuerCert(),
      throwsA(isA<ServerException>()),
    );
  });

  test(
    'a near-expiry stored cert with the server offline is returned',
    () async {
      final stored = _certBytes(_nowUnix + 3 * _daySeconds);
      _certFile(dir).writeAsBytesSync(stored);
      client.trustError = const ServerException(ServerErrorCode.unreachable);

      expect(await trust.issuerCert(), stored);
      expect(_certFile(dir).readAsBytesSync(), stored);
    },
  );

  test('a refetch root mismatch throws and keeps the stored cert', () async {
    final stored = _certBytes(_nowUnix + 3 * _daySeconds);
    _certFile(dir).writeAsBytesSync(stored);
    client.serverKey = ServerKeyResponse(
      rootPublicKey: Uint8List.fromList(List<int>.filled(32, 1)),
      issuerCert: _certBytes(_nowUnix + 30 * _daySeconds),
    );

    await expectLater(trust.issuerCert(), throwsStateError);
    expect(_certFile(dir).readAsBytesSync(), stored);
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
