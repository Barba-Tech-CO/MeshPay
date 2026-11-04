import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/issuer_config_helper.dart';

void main() {
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
