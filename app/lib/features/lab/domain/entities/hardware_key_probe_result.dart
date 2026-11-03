import 'dart:typed_data';

import 'package:meshpay_native/key_security_level.dart';

class HardwareKeyProbeResult {
  const HardwareKeyProbeResult({
    required this.securityLevel,
    required this.hasStrongBox,
    required this.apiLevel,
    required this.chainLength,
    required this.publicKey,
    required this.signatureLength,
  });

  final KeySecurityLevel securityLevel;
  final bool hasStrongBox;
  final int apiLevel;
  final int chainLength;
  final Uint8List publicKey;
  final int signatureLength;
}
