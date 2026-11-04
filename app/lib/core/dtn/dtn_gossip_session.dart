import 'dart:typed_data';

import '../tx/lab_frame_helper.dart';
import '../tx/tx_error_code.dart';
import '../tx/tx_error_message_helper.dart';
import '../tx/tx_lab_exception.dart';
import '../tx/tx_transport_failure.dart';
import 'dtn_buffer_service.dart';
import 'dtn_error_message_helper.dart';
import 'dtn_gossip_stats.dart';
import 'dtn_message_helper.dart';
import 'dtn_rejected_exception.dart';
import 'dtn_session_service.dart';
import 'dtn_test_payload_helper.dart';

/// Foreground gossip over an established Noise session (PRD 8.1), both roles.
/// The initiator drives [sync]: it swaps summaries, then trades one envelope
/// each way per round (empty = nothing left) until both sides are drained,
/// then the snapshots. The responder answers each message through [handle].
/// One connection = one engine [DtnSessionService] (20 attempts), reset by
/// [startConnection]; the counters in [stats] are cumulative.
class DtnGossipSession {
  DtnGossipSession(this._buffer, this._onChanged);

  static const int maximumEnvelopes = 20;

  /// Each side sends at most [maximumEnvelopes], so a legitimate exchange
  /// takes at most one round more than that (the final empty round). Both
  /// roles enforce it: the initiator stops looping, the responder cuts the peer
  /// off with a fixed error.
  static const int maximumRounds = maximumEnvelopes + 1;

  final DtnBufferService _buffer;
  final void Function() _onChanged;
  DtnGossipStats stats = const DtnGossipStats();
  String? error;
  DtnSessionService? _connection;
  List<Uint8List> _outbound = const <Uint8List>[];
  int _cursor = 0;
  int _rounds = 0;
  bool _isSummarized = false;
  Uint8List _pairKey = Uint8List(0);

  void startConnection() {
    _connection = null;
    _outbound = const <Uint8List>[];
    _cursor = 0;
    _rounds = 0;
    _isSummarized = false;
  }

  /// Stores a new random 1-4 KB test envelope in the local buffer.
  Future<void> createTestEnvelope() async {
    error = null;
    try {
      await _buffer.addTestEnvelope(DtnTestPayloadHelper.random());
    } on DtnRejectedException catch (rejection) {
      error = DtnErrorMessageHelper.messageFor(rejection.reason);
    } on Object catch (failure) {
      error = TxErrorMessageHelper.messageFor(failure);
    }
    await refreshHeld();
  }

  Future<void> refreshHeld() async {
    try {
      stats = stats.copyWith(held: (await _buffer.summary()).length);
    } on Object catch (failure) {
      error ??= TxErrorMessageHelper.messageFor(failure);
    }
    _onChanged();
  }

  /// Initiator side; [exchange] sends one plaintext frame and returns the
  /// decrypted reply. Returns true when the link was lost.
  /// [pairKey] is the established link's Noise peer pair key.
  Future<bool> sync(
    Future<Uint8List> Function(Uint8List plaintext) exchange,
    Uint8List pairKey,
  ) async {
    if (_isSummarized) {
      error =
          'Esta conexão já sincronizou. Reconecte para sincronizar de novo.';
      _onChanged();
      return false;
    }
    error = null;
    _pairKey = pairKey;
    _onChanged();
    var linkLost = false;
    try {
      await _runSync(exchange);
    } on TxTransportFailure {
      linkLost = true;
      error = 'O link foi interrompido durante a sincronização. Reconecte.';
    } on Object catch (failure) {
      error = TxErrorMessageHelper.messageFor(failure);
    }
    await refreshHeld();
    return linkLost;
  }

  /// Responder side: handles one DTN frame payload and answers through [send].
  Future<void> handle(
    Uint8List dtnFramePayload,
    Future<void> Function(Uint8List plaintext) send,
    Uint8List pairKey,
  ) async {
    _pairKey = pairKey;
    final Uint8List reply;
    try {
      final message = DtnMessageHelper.decode(dtnFramePayload);
      reply = switch (message.kind) {
        DtnMessageKind.summary => await _answerSummary(message.payload),
        DtnMessageKind.envelope => await _answerEnvelope(message.payload),
        DtnMessageKind.snapshot => await _answerSnapshot(message.payload),
      };
    } on Object catch (failure) {
      error = TxErrorMessageHelper.messageFor(failure);
      _onChanged();
      await send(
        LabFrameHelper.encodeError(TxErrorMessageHelper.codeFor(failure)),
      );
      return;
    }
    await refreshHeld();
    await send(reply);
  }

  Future<void> _runSync(
    Future<Uint8List> Function(Uint8List plaintext) exchange,
  ) async {
    Future<Uint8List> ask(DtnMessageKind kind, Uint8List payload) async {
      final Uint8List plaintext;
      try {
        plaintext = await exchange(DtnMessageHelper.encode(kind, payload));
      } on Object catch (failure) {
        throw TxTransportFailure(failure);
      }
      final frame = LabFrameHelper.decode(plaintext);
      switch (frame.type) {
        case LabFrameType.error:
          throw TxLabException(
            LabFrameHelper.errorCodeOf(frame.payload),
            isRemote: true,
          );
        case LabFrameType.dtn:
          final reply = DtnMessageHelper.decode(frame.payload);
          if (reply.kind != kind) {
            throw const TxLabException(TxErrorCode.invalidMessage);
          }
          return reply.payload;
        case LabFrameType.echo || LabFrameType.tx:
          throw const TxLabException(TxErrorCode.invalidMessage);
      }
    }

    final connection = _connection ??= await _buffer.openSession(
      originPair: _pairKey,
    );
    _isSummarized = true;
    final peerSummary = DtnMessageHelper.decodeSummary(
      await ask(
        DtnMessageKind.summary,
        DtnMessageHelper.encodeSummary(await _buffer.summary()),
      ),
    );
    await _planOutbound(peerSummary);
    var isDrained = false;
    for (var round = 0; round < maximumRounds; round++) {
      final mine = _nextOutbound();
      final theirs = await ask(
        DtnMessageKind.envelope,
        mine ?? Uint8List(0),
      );
      if (theirs.isNotEmpty) {
        await _ingest(connection, theirs);
      }
      if (mine == null && theirs.isEmpty) {
        isDrained = true;
        break;
      }
    }
    if (!isDrained) {
      stats = stats.copyWith(cutSyncs: stats.cutSyncs + 1);
      error = 'A troca excedeu o limite de rodadas e foi interrompida.';
      return;
    }
    final peerSnapshot = await ask(
      DtnMessageKind.snapshot,
      await _buffer.snapshotBytes() ?? Uint8List(0),
    );
    if (peerSnapshot.isNotEmpty) {
      await _ingestSnapshot(peerSnapshot);
    }
  }

  Future<Uint8List> _answerSummary(Uint8List payload) async {
    final peerSummary = DtnMessageHelper.decodeSummary(payload);
    if (_isSummarized) {
      throw const TxLabException(TxErrorCode.dtnSummaryRepeated);
    }
    _isSummarized = true;
    _connection ??= await _buffer.openSession(originPair: _pairKey);
    await _planOutbound(peerSummary);
    return DtnMessageHelper.encode(
      DtnMessageKind.summary,
      DtnMessageHelper.encodeSummary(await _buffer.summary()),
    );
  }

  Future<Uint8List> _answerEnvelope(Uint8List payload) async {
    final connection =
        _connection ?? (throw const TxLabException(TxErrorCode.invalidMessage));
    if (++_rounds > maximumRounds) {
      throw const TxLabException(TxErrorCode.dtnRoundLimit);
    }
    if (payload.isNotEmpty) {
      await _ingest(connection, payload);
    }
    return DtnMessageHelper.encode(
      DtnMessageKind.envelope,
      _nextOutbound() ?? Uint8List(0),
    );
  }

  Future<Uint8List> _answerSnapshot(Uint8List payload) async {
    final own = await _buffer.snapshotBytes() ?? Uint8List(0);
    if (payload.isNotEmpty) {
      await _ingestSnapshot(payload);
    }
    return DtnMessageHelper.encode(DtnMessageKind.snapshot, own);
  }

  Future<void> _planOutbound(List<Uint8List> peerSummary) async {
    _outbound = (await _buffer.missingFrom(
      peerSummary,
    )).take(maximumEnvelopes).toList();
    _cursor = 0;
  }

  Uint8List? _nextOutbound() {
    if (_cursor >= _outbound.length) {
      return null;
    }
    stats = stats.copyWith(sent: stats.sent + 1);
    return _outbound[_cursor++];
  }

  Future<void> _ingest(DtnSessionService connection, Uint8List envelope) async {
    try {
      final isNew = await connection.ingest(envelope);
      stats = isNew
          ? stats.copyWith(received: stats.received + 1)
          : stats.copyWith(duplicates: stats.duplicates + 1);
    } on DtnRejectedException catch (rejection) {
      error = DtnErrorMessageHelper.messageFor(rejection.reason);
      stats = stats.copyWith(rejected: stats.rejected + 1);
    }
  }

  Future<void> _ingestSnapshot(Uint8List snapshot) async {
    final version = await _buffer.ingestSnapshot(snapshot);
    stats = version == null
        ? stats.copyWith(snapshotsRefused: stats.snapshotsRefused + 1)
        : stats.copyWith(snapshotVersion: version);
  }
}
