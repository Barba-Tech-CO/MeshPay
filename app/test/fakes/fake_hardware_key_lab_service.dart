import 'dart:typed_data';

import 'package:meshpay/core/keys/hardware_key_lab_service.dart';
import 'package:meshpay_native/attested_payment_key.dart';
import 'package:meshpay_native/device_capabilities.dart';
import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/payment_key_exception.dart';

class FakeHardwareKeyLabService implements HardwareKeyLabService {
  final List<String> calls = <String>[];
  bool hasStrongBox = false;
  Object? generateError;

  /// One entry per delete call, in order; null or exhausted means success.
  final List<Object?> deleteErrors = <Object?>[];
  bool lastRequireStrongBox = false;

  @override
  Future<DeviceCapabilities> getDeviceCapabilities() async {
    calls.add('capabilities');
    return DeviceCapabilities(
      hasStrongBox: hasStrongBox,
      apiLevel: 34,
      supportsMaxUsageCount: true,
      hasSingleUseKeyFeature: false,
      hasLimitedUseKeyFeature: false,
    );
  }

  @override
  Future<AttestedPaymentKey> generateAttestedKey({
    required String alias,
    required Uint8List challenge,
    required bool requireStrongBox,
  }) async {
    calls.add('generate:$alias');
    lastRequireStrongBox = requireStrongBox;
    if (requireStrongBox && !hasStrongBox) {
      throw const PaymentKeyException('strongbox_unavailable', 'none');
    }
    if (generateError case final error?) {
      throw error;
    }
    return AttestedPaymentKey(
      publicKey: Uint8List.fromList(<int>[
        4,
        ...List<int>.filled(64, alias.contains('payee') ? 173 : 171),
      ]),
      certificateChain: <Uint8List>[Uint8List(3), Uint8List(3), Uint8List(3)],
      securityLevel: requireStrongBox
          ? KeySecurityLevel.strongBox
          : KeySecurityLevel.tee,
    );
  }

  @override
  Future<Uint8List> sign({
    required String alias,
    required Uint8List data,
  }) async {
    calls.add('sign:$alias');
    return Uint8List(71);
  }

  @override
  Future<void> deleteKey({required String alias}) async {
    calls.add('delete:$alias');
    final error = deleteErrors.isEmpty ? null : deleteErrors.removeAt(0);
    if (error != null) {
      throw error;
    }
  }
}
