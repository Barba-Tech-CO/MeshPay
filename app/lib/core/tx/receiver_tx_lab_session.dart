import 'dart:typed_data';

import 'lab_frame_helper.dart';
import 'receiver_tx_engine_service.dart';
import 'tx_error_code.dart';
import 'tx_error_message_helper.dart';
import 'tx_id_helper.dart';
import 'tx_lab_exception.dart';
import 'tx_lab_message_helper.dart';
import 'tx_lab_state.dart';
import 'tx_wire_helper.dart';

/// Receiver side of the lab cheque: HELLO (once per session) opens the engine
/// with the payer key, PROPOSE runs the engine and answers with the signed ack.
/// Failures go back to the payer as fixed error codes, never raw text.
class ReceiverTxLabSession {
  ReceiverTxLabSession(this._engine, this._onChanged);

  static const int publicKeyBytes = 33;

  final ReceiverTxEngineService _engine;
  final void Function() _onChanged;
  TxLabState state = TxLabState.idle;
  String? txIdHex;
  String? error;
  bool _isOpen = false;
  int _generation = 0;

  void reset() {
    state = TxLabState.idle;
    txIdHex = null;
    error = null;
    _isOpen = false;
    _generation++;
  }

  /// Handles one TX frame payload and sends the reply through [send].
  Future<void> handle(
    Uint8List txFramePayload,
    Future<void> Function(Uint8List plaintext) send,
  ) async {
    final Uint8List reply;
    try {
      final message = TxLabMessageHelper.decode(txFramePayload);
      reply = switch (message.kind) {
        TxLabMessageKind.hello => await _answerHello(message.payload),
        TxLabMessageKind.propose => await _answerPropose(message.payload),
        _ => throw const TxLabException(TxErrorCode.invalidMessage),
      };
    } on Object catch (failure) {
      state = TxLabState.failed;
      error = TxErrorMessageHelper.messageFor(failure);
      _onChanged();
      await send(
        LabFrameHelper.encodeError(TxErrorMessageHelper.codeFor(failure)),
      );
      return;
    }
    // The ack is already persisted and signed: the cheque is committed here
    // even if the reply never reaches the payer (it redelivers and gets the
    // same ack).
    if (state == TxLabState.proposed) {
      state = TxLabState.committed;
      _onChanged();
    }
    await send(reply);
  }

  Future<void> close() async {
    _isOpen = false;
    _generation++;
    await _engine.close();
  }

  Future<Uint8List> _answerHello(Uint8List payerPublicKey) async {
    if (_isOpen) {
      throw const TxLabException(TxErrorCode.helloRepeated);
    }
    if (payerPublicKey.length != publicKeyBytes) {
      throw const TxLabException(TxErrorCode.invalidKey);
    }
    final generation = _generation;
    await _engine.open(payerPublicKey: payerPublicKey);
    if (generation != _generation) {
      // Closed or reset while opening: release the handle just created.
      await _engine.close();
      throw StateError('tx session closed while opening');
    }
    _isOpen = true;
    return TxLabMessageHelper.encode(
      TxLabMessageKind.helloReply,
      await _engine.getPublicKey(),
    );
  }

  Future<Uint8List> _answerPropose(Uint8List propose) async {
    if (!_isOpen) {
      throw const TxLabException(TxErrorCode.helloMissing);
    }
    state = TxLabState.proposed;
    error = null;
    _onChanged();
    final ack = await _engine.onPropose(propose);
    txIdHex = TxIdHelper.shortHex(TxWireHelper.txIdFromAck(ack));
    return TxLabMessageHelper.encode(TxLabMessageKind.ack, ack);
  }
}
