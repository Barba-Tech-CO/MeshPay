import 'dart:typed_data';

import '../noise/noise_l2cap_session.dart';
import 'lab_frame_helper.dart';
import 'payer_tx_engine_service.dart';
import 'random_bytes_helper.dart';
import 'tx_error_code.dart';
import 'tx_error_message_helper.dart';
import 'tx_id_helper.dart';
import 'tx_lab_exception.dart';
import 'tx_lab_message_helper.dart';
import 'tx_lab_state.dart';
import 'tx_transport_failure.dart';
import 'tx_wire_helper.dart';

/// Payer side of the lab cheque over one established Noise session: HELLO swaps
/// the public keys inside the encrypted channel, then propose (durable LOCKED)
/// -> TX_PROPOSE -> ack -> COMMITTED. One flow per session.
///
/// LAB: the payer key is NOT bound to an identity. Noise XK authenticates the
/// receiver (pinned by the QR key) but not the initiator, so the receiver only
/// learns "some holder of this key".
class PayerTxLabFlow {
  PayerTxLabFlow({
    required this._session,
    required this._engine,
    required this._onChanged,
  });

  static const int testAmountUnits = 100;
  static const String testUnit = 'TEST';
  static const int publicKeyBytes = 33;
  static const int _chequeIdBytes = 16;

  final NoiseL2capSession _session;
  final PayerTxEngineService _engine;
  final void Function() _onChanged;
  TxLabState state = TxLabState.idle;
  String? txIdHex;
  String? error;
  Uint8List? _payeeKey;
  bool _isRunning = false;
  bool _isCancelled = false;

  /// Stops after the in-flight step; the engine closes once it settles.
  Future<void> cancel() async {
    _isCancelled = true;
    if (!_isRunning) {
      await _engine.close();
    }
  }

  /// Returns true when the link was lost (caller must tear the session down).
  Future<bool> sendCheque() => _run(() async {
    final payee = await _ensurePayeeKey();
    final counter = await _engine.nextCounter();
    _ensureActive();
    final propose = await _engine.propose(
      spendBody: TxWireHelper.buildSpendBody(
        chequeId: RandomBytesHelper.secure(_chequeIdBytes),
        payeeAccountId: payee,
        amountUnits: testAmountUnits,
        counter: counter,
        signedAtUnix: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      ),
      counter: counter,
      recipientId: payee,
    );
    _ensureActive();
    await _lockAndDeliver(propose);
  });

  /// Redelivers the stored TX_PROPOSE bytes of every still-locked cheque and
  /// stops at the first failure.
  Future<bool> resendPending() => _run(() async {
    await _ensurePayeeKey();
    final pending = await _engine.resumePending();
    _ensureActive();
    if (pending.isEmpty) {
      error = 'Nenhum cheque pendente para reenviar.';
      return;
    }
    for (final propose in pending) {
      await _lockAndDeliver(propose);
    }
  });

  Future<bool> _run(Future<void> Function() action) async {
    _isRunning = true;
    error = null;
    _onChanged();
    var linkLost = false;
    try {
      await action();
    } on TxTransportFailure {
      linkLost = true;
      if (!_isCancelled) {
        state = TxLabState.locked;
        error =
            'O link foi interrompido durante a troca. '
            'Reconecte e use Reenviar pendente.';
      }
    } on Object catch (failure) {
      if (!_isCancelled) {
        state = TxLabState.failed;
        error = TxErrorMessageHelper.messageFor(failure);
      }
    } finally {
      _isRunning = false;
      if (_isCancelled) {
        await _engine.close();
      }
    }
    _onChanged();
    return linkLost;
  }

  Future<Uint8List> _ensurePayeeKey() async {
    final known = _payeeKey;
    if (known != null) {
      return known;
    }
    final reply = await _exchange(
      TxLabMessageKind.hello,
      await _engine.getPublicKey(),
    );
    if (reply.kind != TxLabMessageKind.helloReply ||
        reply.payload.length != publicKeyBytes) {
      throw const TxLabException(TxErrorCode.invalidKey);
    }
    _ensureActive();
    await _engine.open(payeePublicKey: reply.payload);
    _ensureActive();
    return _payeeKey = reply.payload;
  }

  Future<void> _lockAndDeliver(Uint8List propose) async {
    txIdHex = TxIdHelper.shortHex(TxWireHelper.txIdFromPropose(propose));
    state = TxLabState.locked;
    _onChanged();
    state = TxLabState.proposed;
    _onChanged();
    final reply = await _exchange(TxLabMessageKind.propose, propose);
    if (reply.kind != TxLabMessageKind.ack) {
      throw const TxLabException(TxErrorCode.invalidMessage);
    }
    await _engine.onAck(reply.payload);
    _ensureActive();
    state = TxLabState.committed;
  }

  Future<({TxLabMessageKind kind, Uint8List payload})> _exchange(
    TxLabMessageKind kind,
    Uint8List payload,
  ) async {
    final Uint8List plaintext;
    try {
      plaintext = await _session.exchangeEncrypted(
        TxLabMessageHelper.encode(kind, payload),
      );
    } on Object catch (failure) {
      throw TxTransportFailure(failure);
    }
    _ensureActive();
    final frame = LabFrameHelper.decode(plaintext);
    switch (frame.type) {
      case LabFrameType.error:
        throw TxLabException(
          LabFrameHelper.errorCodeOf(frame.payload),
          isRemote: true,
        );
      case LabFrameType.tx:
        return TxLabMessageHelper.decode(frame.payload);
      case LabFrameType.echo || LabFrameType.dtn:
        throw const TxLabException(TxErrorCode.invalidMessage);
    }
  }

  void _ensureActive() {
    if (_isCancelled) {
      throw StateError('tx flow cancelled');
    }
  }
}
