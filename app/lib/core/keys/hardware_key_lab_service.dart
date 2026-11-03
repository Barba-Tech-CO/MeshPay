import 'dart:typed_data';

import 'package:meshpay_native/attested_payment_key.dart';
import 'package:meshpay_native/device_capabilities.dart';

/// Keystore operations used by the hardware key lab page.
abstract interface class HardwareKeyLabService {
  Future<DeviceCapabilities> getDeviceCapabilities();

  Future<AttestedPaymentKey> generateAttestedKey({
    required String alias,
    required Uint8List challenge,
    required bool requireStrongBox,
  });

  Future<Uint8List> sign({required String alias, required Uint8List data});

  Future<void> deleteKey({required String alias});
}
