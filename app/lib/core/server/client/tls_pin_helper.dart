import 'dart:convert';
import 'dart:io';

import 'package:dio/io.dart';
import 'package:flutter/foundation.dart';

import 'server_base_url_helper.dart';

class TlsPinHelper {
  TlsPinHelper._();

  static const String _serverBaseUrl = String.fromEnvironment(
    'SERVER_BASE_URL',
  );
  static const String _serverCaPemB64 = String.fromEnvironment(
    'SERVER_CA_PEM_B64',
  );

  static IOHttpClientAdapter adapterForPem(Uint8List pem) =>
      IOHttpClientAdapter(
        createHttpClient: () => HttpClient(
          context: SecurityContext(withTrustedRoots: false)
            ..setTrustedCertificatesBytes(pem),
        ),
      );

  static IOHttpClientAdapter? configuredAdapter({
    bool isRelease = kReleaseMode,
    String configuredBaseUrl = _serverBaseUrl,
    String configuredCaPemB64 = _serverCaPemB64,
  }) {
    if (isRelease &&
        !ServerBaseUrlHelper.isAllowed(configuredBaseUrl, isRelease: true)) {
      throw StateError(
        'Release build requires an HTTPS --dart-define=SERVER_BASE_URL.',
      );
    }

    if (configuredCaPemB64.isEmpty) {
      if (isRelease) {
        throw StateError(
          'Release build requires --dart-define=SERVER_CA_PEM_B64.',
        );
      }
      return null;
    }

    try {
      final pem = Uint8List.fromList(base64.decode(configuredCaPemB64));
      if (pem.isEmpty) {
        throw StateError(
          'SERVER_CA_PEM_B64 must contain a base64-encoded PEM CA.',
        );
      }
      return adapterForPem(pem);
    } on FormatException {
      throw StateError('SERVER_CA_PEM_B64 must be valid base64.');
    }
  }
}
