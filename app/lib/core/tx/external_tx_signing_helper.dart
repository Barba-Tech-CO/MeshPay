import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../keys/payment_key_signer.dart';

/// Two-phase external signing: prepare in Rust, sign in the Keystore, finish
/// in Rust. A stored propose/ack (redelivery) is returned without signing.
class ExternalTxSigningHelper {
  ExternalTxSigningHelper._();

  static Future<Uint8List> propose({
    required PayerTxEngine engine,
    required PaymentKeySigner signer,
    required String alias,
    required Uint8List spendBody,
    required BigInt counter,
    required Uint8List recipientId,
  }) async {
    final prepared = await engine.preparePropose(
      spendBody: spendBody,
      counter: counter,
      recipientId: recipientId,
    );
    final existing = prepared.existingPropose;
    if (existing != null) {
      return existing;
    }
    final signature = await signer.sign(alias, prepared.message);
    return engine.finishPropose(prepared: prepared, signature: signature);
  }

  static Future<Uint8List> acknowledge({
    required ReceiverTxEngine engine,
    required PaymentKeySigner signer,
    required String alias,
    required Uint8List propose,
  }) async {
    final txId = await engine.storePropose(propose: propose);
    final prepared = await engine.prepareAck(txId: txId);
    final existing = prepared.existingAck;
    if (existing != null) {
      return existing;
    }
    final signature = await signer.sign(alias, prepared.message);
    return engine.finishAck(txId: prepared.txId, signature: signature);
  }
}
