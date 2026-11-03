import 'dart:typed_data';

import 'key_security_level.dart';
import 'native_payload.dart';

class AttestedPaymentKey {
  const AttestedPaymentKey({
    required this.publicKey,
    required this.certificateChain,
    required this.securityLevel,
  });

  /// Uncompressed SEC1 P-256 point: 0x04 || X || Y (65 bytes).
  final Uint8List publicKey;

  /// DER certificates, leaf first.
  final List<Uint8List> certificateChain;
  final KeySecurityLevel securityLevel;

  factory AttestedPaymentKey.fromPlatform(Object? value) {
    final payload = NativePayload.readMap(value, 'attested payment key');
    return AttestedPaymentKey(
      publicKey: NativePayload.readBytesField(payload, 'publicKey'),
      certificateChain: NativePayload.readBytesList(
        payload['certificateChain'],
        'certificateChain',
      ),
      securityLevel: KeySecurityLevel.fromPlatform(payload['securityLevel']),
    );
  }
}
