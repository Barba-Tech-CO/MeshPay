import 'dart:typed_data';

import '../messages/cheque_spend.dart';
import '../messages/cheque_spend_body.dart';
import 'server_signing_message_helper.dart';
import 'spend_tx_id_helper.dart';

/// Builds the spend body (deriving its tx_id) and signs it with the payer key.
class ServerSpendHelper {
  ServerSpendHelper._();

  /// [sign] returns the raw r||s signature over the given message.
  static Future<ChequeSpend> build({
    required Uint8List chequeId,
    required Uint8List payeeAccountId,
    required int amountUnits,
    required int counter,
    required int signedAtUnix,
    required Future<Uint8List> Function(Uint8List message) sign,
  }) async {
    final unsigned = ChequeSpendBody(
      chequeId: chequeId,
      payeeAccountId: payeeAccountId,
      amountUnits: amountUnits,
      txId: Uint8List(0),
      counter: counter,
      prevSpendHash: Uint8List(0),
      signedAtUnix: signedAtUnix,
    );
    final body = unsigned
        .withTxId(SpendTxIdHelper.derive(counter, unsigned.toBytes()))
        .toBytes();
    return ChequeSpend(
      body: body,
      payerSignature: await sign(ServerSigningMessageHelper.spend(body)),
    );
  }
}
