import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/snapshot_config_helper.dart';
import 'package:meshpay_core/meshpay_core.dart' show RustLib;
import 'package:meshpay_core/src/rust/frb_generated.dart' show RustLibApi;

void main() {
  setUpAll(() => RustLib.initMock(api: _FakeRustLibApi()));

  test('debug snapshot cert defaults to lab cert', () async {
    final cert = await SnapshotConfigHelper.cert(isRelease: false);

    expect(cert, isNotEmpty);
  });

  test('release snapshot cert fails closed without configuration', () async {
    await expectLater(
      SnapshotConfigHelper.cert(isRelease: true, configuredHex: ''),
      throwsStateError,
    );
  });

  test(
    'release snapshot cert can be fetched from pinned server trust',
    () async {
      final cert = await SnapshotConfigHelper.cert(
        isRelease: true,
        configuredHex: '',
        fetch: () async => Uint8List.fromList(<int>[1, 2, 3]),
      );

      expect(cert, <int>[1, 2, 3]);
    },
  );
}

class _FakeRustLibApi implements RustLibApi {
  @override
  Future<Uint8List> crateApiLabSnapshotCert({
    required BigInt notBeforeUnix,
  }) async => Uint8List.fromList(<int>[1, 2, 3]);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
