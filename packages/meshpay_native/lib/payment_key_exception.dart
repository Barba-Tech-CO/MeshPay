import 'package:flutter/services.dart';

/// Keystore failure with the stable native [code]: alias_exists,
/// invalid_alias, invalid_count, data_too_large, attestation_unavailable,
/// strongbox_unavailable, max_usage_count_unsupported, key_not_found,
/// key_unusable, sign_failed, keystore_error, plugin_detached.
class PaymentKeyException implements Exception {
  const PaymentKeyException(this.code, this.message);

  final String code;
  final String? message;

  factory PaymentKeyException.fromPlatform(PlatformException error) {
    return PaymentKeyException(error.code, error.message);
  }

  @override
  String toString() => 'PaymentKeyException($code): $message';
}
