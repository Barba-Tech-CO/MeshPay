import 'dart:typed_data';

import 'package:meshpay_native/key_security_level.dart';

class PaymentKeyIdentity {
  const PaymentKeyIdentity({
    required this.publicKey,
    required this.securityLevel,
  });

  /// Uncompressed SEC1 P-256 point (65 bytes).
  final Uint8List publicKey;
  final KeySecurityLevel securityLevel;
}
