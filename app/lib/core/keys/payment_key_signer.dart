import 'dart:typed_data';

import 'payment_key_identity.dart';

/// Hardware-backed signer (Android Keystore) addressed by logical alias.
abstract interface class PaymentKeySigner {
  /// Returns the public key of [alias], generating it on first use. Never
  /// changes the identity of a key whose engine database ([dbPath]) exists:
  /// it throws a `TxLabException` with `localKeyInconsistent` instead.
  Future<PaymentKeyIdentity> ensureKey(
    String alias, {
    required String dbPath,
  });

  /// DER ECDSA signature of [message] (the Keystore hashes with SHA-256).
  Future<Uint8List> sign(String alias, Uint8List message);
}
