import 'dart:io';
import 'dart:typed_data';

import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/meshpay_native.dart';
import 'package:meshpay_native/payment_key_exception.dart';

import '../storage/app_support_directory_service.dart';
import '../tx/lab_public_key_helper.dart';
import '../tx/random_bytes_helper.dart';
import '../tx/tx_error_code.dart';
import '../tx/tx_lab_exception.dart';
import 'payment_key_identity.dart';
import 'payment_key_signer.dart';

/// LAB: the Keystore cannot return the public key of an existing alias, so it
/// is kept in `meshpay_key_<alias>.pub` next to the app data (public data).
/// Key and file are only regenerated while no engine database exists for the
/// alias; afterwards a mismatch is an error, never a silent identity change.
class MeshpayNativePaymentKeySigner implements PaymentKeySigner {
  MeshpayNativePaymentKeySigner(this._native, this._directory);

  static const int _challengeBytes = 32;
  static const String _keyNotFound = 'key_not_found';

  final MeshpayNative _native;
  final AppSupportDirectoryService _directory;
  final Map<String, Future<PaymentKeyIdentity>> _inFlight =
      <String, Future<PaymentKeyIdentity>>{};

  @override
  Future<PaymentKeyIdentity> ensureKey(
    String alias, {
    required String dbPath,
  }) => _inFlight[alias] ??=
      _resolve(
        alias,
        dbPath,
      ).whenComplete(() {
        _inFlight.remove(alias);
      });

  Future<PaymentKeyIdentity> _resolve(String alias, String dbPath) async {
    final file = File('${await _directory.getPath()}/meshpay_key_$alias.pub');
    final stored = await _readValid(file);
    final level = await _securityLevelOrNull(alias);
    if (stored != null && level != null) {
      return PaymentKeyIdentity(publicKey: stored, securityLevel: level);
    }
    if (File(dbPath).existsSync()) {
      throw const TxLabException(TxErrorCode.localKeyInconsistent);
    }
    if (level != null) {
      await _native.deletePaymentKey(alias: alias);
    }
    final key = await _native.generateAttestedPaymentKey(
      alias: alias,
      challenge: RandomBytesHelper.secure(_challengeBytes),
      requireStrongBox: false,
    );
    await _writeAtomic(file, key.publicKey);
    return PaymentKeyIdentity(
      publicKey: key.publicKey,
      securityLevel: key.securityLevel,
    );
  }

  @override
  Future<Uint8List> sign(String alias, Uint8List message) =>
      _native.signWithPaymentKey(alias: alias, data: message);

  Future<KeySecurityLevel?> _securityLevelOrNull(String alias) async {
    try {
      return (await _native.getPaymentKeyInfo(alias: alias)).securityLevel;
    } on PaymentKeyException catch (failure) {
      if (failure.code == _keyNotFound) {
        return null;
      }
      rethrow;
    }
  }

  Future<Uint8List?> _readValid(File file) async {
    if (!file.existsSync()) {
      return null;
    }
    final bytes = await file.readAsBytes();
    return LabPublicKeyHelper.isValid(bytes) ? bytes : null;
  }

  Future<void> _writeAtomic(File file, Uint8List bytes) async {
    final temp = File('${file.path}.tmp');
    await temp.writeAsBytes(bytes, flush: true);
    await temp.rename(file.path);
  }
}
