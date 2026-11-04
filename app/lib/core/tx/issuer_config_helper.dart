import 'package:flutter/foundation.dart';

import '../noise/hex_helper.dart';
import 'lab_cheque_stub_helper.dart';

/// Issuer trust anchor for receivers. Release builds read the operator values
/// from `--dart-define` and fail closed; non-release builds can override the
/// public lab root with the same define.
class IssuerConfigHelper {
  IssuerConfigHelper._();

  static const String _rootHex = String.fromEnvironment(
    'ISSUER_ROOT_PUBLIC_KEY_HEX',
  );
  static const String _certHex = String.fromEnvironment('ISSUER_CERT_HEX');

  static Future<Uint8List> rootPublicKey({
    bool isRelease = kReleaseMode,
    String configuredHex = _rootHex,
  }) async {
    if (!isRelease && configuredHex.isEmpty) {
      return LabChequeStubHelper.issuerRootPublicKey();
    }
    return _decode('ISSUER_ROOT_PUBLIC_KEY_HEX', configuredHex);
  }

  static Future<Uint8List> cert({
    bool isRelease = kReleaseMode,
    String configuredHex = _certHex,
    Future<Uint8List> Function()? fetch,
  }) async {
    if (!isRelease) return LabChequeStubHelper.issuerCert();
    if (configuredHex.isEmpty && fetch != null) return fetch();
    return _decode('ISSUER_CERT_HEX', configuredHex);
  }

  static Uint8List _decode(String name, String value) {
    if (value.isEmpty) {
      throw StateError(
        'Release build requires --dart-define=$name; refusing the lab issuer.',
      );
    }
    return HexHelper.decode(value);
  }
}
