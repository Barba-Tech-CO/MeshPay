import 'package:meshpay_native/key_security_level.dart';

/// Mirror of server/src/wire/attestation_level.rs.
enum ServerAttestationLevel {
  unspecified(0),
  none(1),
  iosAppAttest(2),
  androidTee(3),
  androidStrongbox(4),
  labSoftware(99);

  const ServerAttestationLevel(this.value);

  final int value;

  static ServerAttestationLevel fromSecurityLevel(KeySecurityLevel level) =>
      switch (level) {
        KeySecurityLevel.strongBox => androidStrongbox,
        KeySecurityLevel.tee => androidTee,
        KeySecurityLevel.software || KeySecurityLevel.unknown => none,
      };

  static ServerAttestationLevel fromValue(int value) =>
      ServerAttestationLevel.values.firstWhere(
        (level) => level.value == value,
        orElse: () => ServerAttestationLevel.unspecified,
      );
}
