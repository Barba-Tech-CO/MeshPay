import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../storage/app_support_directory_service.dart';
import 'random_bytes_helper.dart';

/// LAB ONLY: per-side database, software keys and cheque counter under the
/// app support dir. The key file (P-256 seed, public key, SQLCipher key) is
/// PLAINTEXT on disk and unprotected; a real build keeps these in the
/// Keystore/Secure Enclave.
class LabTxStorageHelper {
  LabTxStorageHelper._();

  static const int _seedBytes = 32;
  static const int _publicKeyBytes = 33;
  static const int _dbKeyBytes = 32;

  static Future<String> dbPath(
    AppSupportDirectoryService directory,
    String side,
  ) async => '${await directory.getPath()}/meshpay_lab_$side.db';

  static Future<Uint8List> publicKey(
    AppSupportDirectoryService directory,
    String side,
  ) async => (await loadOrCreateKeys(directory, side)).publicKey;

  /// Generates the keys on first use and reuses the stored ones afterwards.
  /// A key file of the wrong length throws: regenerating would orphan the
  /// database it encrypts.
  static Future<({Uint8List seed, Uint8List publicKey, Uint8List dbKey})>
  loadOrCreateKeys(AppSupportDirectoryService directory, String side) async {
    final file = File('${await directory.getPath()}/meshpay_lab_$side.keys');
    if (file.existsSync()) {
      final bytes = await file.readAsBytes();
      if (bytes.length != _seedBytes + _publicKeyBytes + _dbKeyBytes) {
        throw FormatException(
          'Arquivo de chaves do laboratório corrompido: ${file.path}',
        );
      }
      return _split(bytes);
    }
    final key = await generateLabSigningKey();
    final bytes = Uint8List.fromList(<int>[
      ...key.seed,
      ...key.publicKey,
      ...RandomBytesHelper.secure(_dbKeyBytes),
    ]);
    await _writeAtomic(file, bytes);
    return _split(bytes);
  }

  /// Persisted, strictly increasing cheque counter (first value 1). Callers
  /// must not run two reads concurrently for the same side.
  static Future<int> nextCounter(
    AppSupportDirectoryService directory,
    String side,
  ) async {
    final file = File('${await directory.getPath()}/meshpay_lab_$side.counter');
    var last = 0;
    if (file.existsSync()) {
      final parsed = int.tryParse(await file.readAsString());
      if (parsed == null || parsed < 0) {
        throw FormatException(
          'Contador do laboratório corrompido: ${file.path}',
        );
      }
      last = parsed;
    }
    final next = last + 1;
    await _writeAtomic(file, utf8.encode('$next'));
    return next;
  }

  static Future<void> _writeAtomic(File file, List<int> bytes) async {
    final temp = File('${file.path}.tmp');
    await temp.writeAsBytes(bytes, flush: true);
    await temp.rename(file.path);
  }

  static ({Uint8List seed, Uint8List publicKey, Uint8List dbKey}) _split(
    Uint8List bytes,
  ) => (
    seed: bytes.sublist(0, _seedBytes),
    publicKey: bytes.sublist(_seedBytes, _seedBytes + _publicKeyBytes),
    dbKey: bytes.sublist(_seedBytes + _publicKeyBytes),
  );
}
