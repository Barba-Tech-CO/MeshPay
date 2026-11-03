import 'native_protocol_exception.dart';

enum KeySecurityLevel {
  software,
  tee,
  strongBox,
  unknown;

  // Below API 31 StrongBox cannot be told apart from TEE; it reports tee.
  static KeySecurityLevel fromPlatform(Object? value) {
    for (final level in KeySecurityLevel.values) {
      if (level.name == value) {
        return level;
      }
    }
    throw NativeProtocolException('Unknown key security level: $value');
  }
}
