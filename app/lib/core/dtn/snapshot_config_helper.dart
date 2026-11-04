import 'package:flutter/foundation.dart';

import '../noise/hex_helper.dart';
import 'lab_dtn_snapshot_cert_helper.dart';

/// Snapshot cert configuration. Release builds require operator cert bytes.
class SnapshotConfigHelper {
  SnapshotConfigHelper._();

  static const String _certHex = String.fromEnvironment('SNAPSHOT_CERT_HEX');

  static Future<Uint8List> cert({
    bool isRelease = kReleaseMode,
    String configuredHex = _certHex,
    Future<Uint8List> Function()? fetch,
  }) async {
    if (!isRelease) return LabDtnSnapshotCertHelper.cert();
    if (configuredHex.isEmpty && fetch != null) return fetch();
    if (configuredHex.isEmpty) {
      throw StateError(
        'Release build requires --dart-define=SNAPSHOT_CERT_HEX; refusing the lab snapshot.',
      );
    }
    return HexHelper.decode(configuredHex);
  }
}
