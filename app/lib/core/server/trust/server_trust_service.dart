import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../../storage/app_support_directory_service.dart';
import '../../tx/issuer_config_helper.dart';
import '../client/server_client.dart';
import '../client/server_error_code.dart';
import '../client/server_exception.dart';
import '../messages/server_key_cert_summary.dart';

/// Fetches issuer trust material from the ledger server. The root key stays
/// pinned by build config: the server's root is only compared to it.
class ServerTrustService {
  ServerTrustService(
    this._client,
    this._directory, {
    this.configuredBaseUrl = const String.fromEnvironment('SERVER_BASE_URL'),
    this.isRelease = kReleaseMode,
    this.pinnedRoot = IssuerConfigHelper.rootPublicKey,
    this.clock = DateTime.now,
  });

  static const String labBaseUrl = 'http://127.0.0.1:18080';
  static const Duration _refreshWindow = Duration(days: 7);
  static int _temporaryWrites = 0;

  final ServerClient _client;
  final AppSupportDirectoryService _directory;
  final String configuredBaseUrl;
  final bool isRelease;
  final Future<Uint8List> Function() pinnedRoot;
  final DateTime Function() clock;

  /// Set once a refetch failed to extend validity, so a server stuck on a
  /// near-expiry cert cannot be polled again by this process.
  bool _refetchWithoutExtension = false;

  /// Release fails closed without `--dart-define=SERVER_BASE_URL`.
  String get baseUrl {
    if (configuredBaseUrl.isNotEmpty) return configuredBaseUrl;
    if (isRelease) {
      throw StateError('Release build requires --dart-define=SERVER_BASE_URL.');
    }
    return labBaseUrl;
  }

  /// Issuer cert: the stored copy while it is fresh (offline receive), else
  /// refetched from the server and replaced atomically. Core `IssuerTrust`
  /// verifies it against the pinned root either way. Throws [StateError] when
  /// the server's root differs; an offline refetch falls back to the stored
  /// cert only while it is still unexpired.
  Future<Uint8List> issuerCert() async {
    final file = File('${await _directory.getPath()}/issuer_cert.bin');
    Uint8List? stored;
    var storedNotAfterUnix = 0;
    if (file.existsSync()) {
      try {
        stored = await file.readAsBytes();
        storedNotAfterUnix = ServerKeyCertSummary.fromBytes(
          stored,
        ).notAfterUnix;
      } on Object {
        stored = null;
      }
    }
    final nowUnix = clock().millisecondsSinceEpoch ~/ 1000;
    if (stored != null &&
        storedNotAfterUnix - nowUnix > _refreshWindow.inSeconds) {
      return stored;
    }
    if (_refetchWithoutExtension &&
        stored != null &&
        storedNotAfterUnix > nowUnix) {
      return stored;
    }
    try {
      final response = await _client.getServerKey(baseUrl);
      if (!listEquals(response.rootPublicKey, await pinnedRoot())) {
        throw StateError('Server issuer root does not match the pinned root.');
      }
      final fetchedNotAfterUnix = ServerKeyCertSummary.fromBytes(
        response.issuerCert,
      ).notAfterUnix;
      if (fetchedNotAfterUnix < storedNotAfterUnix) {
        _refetchWithoutExtension = true;
        return _unexpiredStoredOrThrow(stored, storedNotAfterUnix, nowUnix);
      }
      await _replace(file, response.issuerCert);
      if (fetchedNotAfterUnix == storedNotAfterUnix) {
        _refetchWithoutExtension = true;
      }
      return response.issuerCert;
    } on TimeoutException {
      if (stored != null && storedNotAfterUnix > nowUnix) return stored;
      rethrow;
    } on ServerException {
      if (stored != null && storedNotAfterUnix > nowUnix) return stored;
      rethrow;
    }
  }

  Uint8List _unexpiredStoredOrThrow(
    Uint8List? stored,
    int storedNotAfterUnix,
    int nowUnix,
  ) {
    if (stored != null && storedNotAfterUnix > nowUnix) return stored;
    throw StateError('No unexpired issuer cert is available.');
  }

  Future<void> _replace(File file, Uint8List cert) async {
    final temporary = File('${file.path}.tmp.${_temporaryWrites++}');
    await temporary.writeAsBytes(cert, flush: true);
    try {
      await temporary.rename(file.path);
    } on Object {
      if (temporary.existsSync()) temporary.deleteSync();
      rethrow;
    }
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
