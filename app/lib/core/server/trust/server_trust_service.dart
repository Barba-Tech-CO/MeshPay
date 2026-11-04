import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../../storage/app_support_directory_service.dart';
import '../../tx/issuer_config_helper.dart';
import '../client/server_client.dart';
import '../client/server_error_code.dart';
import '../client/server_exception.dart';

/// Fetches issuer trust material from the ledger server. The root key stays
/// pinned by build config: the server's root is only compared to it.
class ServerTrustService {
  ServerTrustService(
    this._client,
    this._directory, {
    this.configuredBaseUrl = const String.fromEnvironment('SERVER_BASE_URL'),
    this.isRelease = kReleaseMode,
    this.pinnedRoot = IssuerConfigHelper.rootPublicKey,
  });

  static const String labBaseUrl = 'http://127.0.0.1:18080';

  final ServerClient _client;
  final AppSupportDirectoryService _directory;
  final String configuredBaseUrl;
  final bool isRelease;
  final Future<Uint8List> Function() pinnedRoot;

  /// Release fails closed without `--dart-define=SERVER_BASE_URL`.
  String get baseUrl {
    if (configuredBaseUrl.isNotEmpty) return configuredBaseUrl;
    if (isRelease) {
      throw StateError('Release build requires --dart-define=SERVER_BASE_URL.');
    }
    return labBaseUrl;
  }

  /// Issuer cert: the stored copy first (offline receive), else fetched from
  /// the server and stored. Core `IssuerTrust` verifies it against the pinned
  /// root either way. Throws [StateError] when the server's root differs.
  Future<Uint8List> issuerCert() async {
    final file = File('${await _directory.getPath()}/issuer_cert.bin');
    if (file.existsSync()) return file.readAsBytes();
    final response = await _client.getServerKey(baseUrl);
    if (!listEquals(response.rootPublicKey, await pinnedRoot())) {
      throw StateError('Server issuer root does not match the pinned root.');
    }
    await file.writeAsBytes(response.issuerCert, flush: true);
    return response.issuerCert;
  }

  /// Latest signed snapshot, or null when it cannot be fetched (the caller
  /// keeps the held one). Non-network failures are logged, not hidden.
  Future<Uint8List?> refreshSnapshot() async {
    final url = baseUrl;
    try {
      return await _client
          .getRevocationSnapshot(url)
          .timeout(const Duration(seconds: 2));
    } on TimeoutException {
      return null;
    } on ServerException catch (failure) {
      if (failure.code != ServerErrorCode.unreachable) {
        debugPrint('snapshot refresh failed: ${failure.code.name}');
      }
      return null;
    }
  }
}
