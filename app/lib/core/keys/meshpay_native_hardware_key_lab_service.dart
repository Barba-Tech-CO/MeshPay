import 'dart:typed_data';

import 'package:meshpay_native/attested_payment_key.dart';
import 'package:meshpay_native/device_capabilities.dart';
import 'package:meshpay_native/meshpay_native.dart';

import 'hardware_key_lab_service.dart';

class MeshpayNativeHardwareKeyLabService implements HardwareKeyLabService {
  MeshpayNativeHardwareKeyLabService(this._native);

  final MeshpayNative _native;

  @override
  Future<DeviceCapabilities> getDeviceCapabilities() =>
      _native.getDeviceCapabilities();

  @override
  Future<AttestedPaymentKey> generateAttestedKey({
    required String alias,
    required Uint8List challenge,
    required bool requireStrongBox,
  }) => _native.generateAttestedPaymentKey(
    alias: alias,
    challenge: challenge,
    requireStrongBox: requireStrongBox,
  );

  @override
  Future<Uint8List> sign({required String alias, required Uint8List data}) =>
      _native.signWithPaymentKey(alias: alias, data: data);

  @override
  Future<void> deleteKey({required String alias}) =>
      _native.deletePaymentKey(alias: alias);
}
