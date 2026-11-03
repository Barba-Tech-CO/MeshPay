import 'dart:typed_data';

import 'package:meshpay/core/keys/payment_key_identity.dart';
import 'package:meshpay/core/keys/payment_key_signer.dart';
import 'package:meshpay_native/key_security_level.dart';

class FakePaymentKeySigner implements PaymentKeySigner {
  final List<String> calls = <String>[];
  final List<Uint8List> signedMessages = <Uint8List>[];
  Uint8List signature = Uint8List.fromList(List<int>.filled(70, 9));
  Object? signError;

  @override
  Future<PaymentKeyIdentity> ensureKey(
    String alias, {
    required String dbPath,
  }) async {
    calls.add('ensureKey:$alias');
    return PaymentKeyIdentity(
      publicKey: Uint8List.fromList(<int>[4, ...List<int>.filled(64, 5)]),
      securityLevel: KeySecurityLevel.tee,
    );
  }

  @override
  Future<Uint8List> sign(String alias, Uint8List message) async {
    calls.add('sign:$alias');
    if (signError case final error?) {
      throw error;
    }
    signedMessages.add(message);
    return signature;
  }
}
