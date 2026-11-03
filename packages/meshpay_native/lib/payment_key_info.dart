import 'key_security_level.dart';
import 'native_payload.dart';

class PaymentKeyInfo {
  const PaymentKeyInfo({
    required this.securityLevel,
    required this.isHardwareBacked,
  });

  final KeySecurityLevel securityLevel;
  final bool isHardwareBacked;

  factory PaymentKeyInfo.fromPlatform(Object? value) {
    final payload = NativePayload.readMap(value, 'payment key info');
    return PaymentKeyInfo(
      securityLevel: KeySecurityLevel.fromPlatform(payload['securityLevel']),
      isHardwareBacked: NativePayload.readBool(payload, 'isHardwareBacked'),
    );
  }
}
