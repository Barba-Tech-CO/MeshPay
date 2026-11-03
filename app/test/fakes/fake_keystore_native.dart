import 'dart:typed_data';

import 'package:meshpay_native/attested_payment_key.dart';
import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/meshpay_native.dart';
import 'package:meshpay_native/payment_key_exception.dart';
import 'package:meshpay_native/payment_key_info.dart';

/// In-memory Keystore: only the payment key calls the signer uses.
class FakeKeystoreNative implements MeshpayNative {
  final Map<String, Uint8List> keys = <String, Uint8List>{};
  final List<String> calls = <String>[];
  int generated = 0;

  @override
  Future<PaymentKeyInfo> getPaymentKeyInfo({required String alias}) async {
    calls.add('info:$alias');
    if (!keys.containsKey(alias)) {
      throw const PaymentKeyException('key_not_found', null);
    }
    return const PaymentKeyInfo(
      securityLevel: KeySecurityLevel.tee,
      isHardwareBacked: true,
    );
  }

  @override
  Future<AttestedPaymentKey> generateAttestedPaymentKey({
    required String alias,
    required Uint8List challenge,
    required bool requireStrongBox,
    bool singleUse = false,
  }) async {
    calls.add('generate:$alias');
    await Future<void>.delayed(Duration.zero);
    if (keys.containsKey(alias)) {
      throw const PaymentKeyException('alias_exists', null);
    }
    generated++;
    final publicKey = Uint8List.fromList(<int>[
      4,
      ...List<int>.filled(64, generated),
    ]);
    keys[alias] = publicKey;
    return AttestedPaymentKey(
      publicKey: publicKey,
      certificateChain: <Uint8List>[Uint8List(1)],
      securityLevel: KeySecurityLevel.tee,
    );
  }

  @override
  Future<void> deletePaymentKey({required String alias}) async {
    calls.add('delete:$alias');
    keys.remove(alias);
  }

  @override
  Future<Uint8List> signWithPaymentKey({
    required String alias,
    required Uint8List data,
  }) async => Uint8List(70);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
