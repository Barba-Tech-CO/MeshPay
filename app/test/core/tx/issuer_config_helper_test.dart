import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/issuer_config_helper.dart';
import 'package:meshpay_core/meshpay_core.dart' show RustLib;
import 'package:meshpay_core/src/rust/frb_generated.dart' show RustLibApi;

void main() {
  setUpAll(() => RustLib.initMock(api: _FakeRustLibApi()));

  test('debug without a configured root uses the lab root', () async {
    expect(
      await IssuerConfigHelper.rootPublicKey(
        isRelease: false,
        configuredHex: '',
      ),
      <int>[1, 2],
    );
  });

  test('debug uses the configured root when provided', () async {
    expect(
      await IssuerConfigHelper.rootPublicKey(
        isRelease: false,
        configuredHex: '0a0b',
      ),
      <int>[10, 11],
    );
  });

  test('release without dart-define fails closed, no lab root', () {
    expect(
      IssuerConfigHelper.rootPublicKey(isRelease: true, configuredHex: ''),
      throwsStateError,
    );
    expect(
      IssuerConfigHelper.cert(isRelease: true, configuredHex: ''),
      throwsStateError,
    );
  });

  test('release uses the configured operator values', () async {
    expect(
      await IssuerConfigHelper.rootPublicKey(
        isRelease: true,
        configuredHex: '0a0b',
      ),
      <int>[10, 11],
    );
    expect(
      await IssuerConfigHelper.cert(isRelease: true, configuredHex: 'ff'),
      <int>[255],
    );
  });
}

class _FakeRustLibApi implements RustLibApi {
  @override
  Future<Uint8List> crateApiLabIssuerRootPublicKey() async =>
      Uint8List.fromList(<int>[1, 2]);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
