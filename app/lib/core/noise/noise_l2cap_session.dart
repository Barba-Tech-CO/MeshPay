import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

import '../ble/l2cap_frame_codec.dart';
import '../ble/l2cap_frame_reader.dart';
import '../ble/native_lab_service.dart';
import 'noise_channel.dart';
import 'noise_handshake_status.dart';

/// One Noise XK session over one L2CAP channel: framing, handshake and
/// encrypted request/response, for both the initiator and the responder.
class NoiseL2capSession {
  NoiseL2capSession({
    required this._nativeService,
    required this.channelId,
    required this._noise,
    this.frameTimeout = const Duration(seconds: 5),
  });

  /// Handshake messages are at most 64 bytes; anything larger is rejected
  /// before the handshake finishes.
  static const int handshakeMaxBodyBytes = 512;

  final int channelId;
  final Duration frameTimeout;
  final NativeLabService _nativeService;
  final NoiseChannel _noise;
  final L2capFrameReader _reader = L2capFrameReader(
    maxBodyBytes: handshakeMaxBodyBytes,
  );
  final Queue<Uint8List> _inbox = Queue<Uint8List>();
  Completer<Uint8List>? _pending;
  NoiseHandshakeStatus status = NoiseHandshakeStatus.inProgress;
  List<int> messageSizes = const <int>[];
  bool isClosed = false;

  bool get isEstablished => _noise.isFinished;

  /// Handshake done and the channel still open.
  bool get isUsable => isEstablished && !isClosed;

  /// Buffers raw stream bytes; throws [FormatException] on an invalid frame.
  void addData(Uint8List chunk) => _reader.add(chunk);

  Uint8List? nextBody() {
    _reader.maxBodyBytes = _noise.isFinished
        ? L2capFrameCodec.maximumBodyBytes
        : handshakeMaxBodyBytes;
    return _reader.nextBody();
  }

  Future<void> send(Uint8List body) => _nativeService.sendL2cap(
    channelId: channelId,
    data: L2capFrameCodec.encode(body),
  );

  /// Initiator side: buffers [chunk] and hands every complete frame to the
  /// pending exchange (extra frames wait for the next one).
  void deliver(Uint8List chunk) {
    addData(chunk);
    for (var body = nextBody(); body != null; body = nextBody()) {
      _inbox.add(body);
    }
    _completePending();
  }

  Future<Uint8List> exchange(Uint8List body) async {
    final pending = Completer<Uint8List>();
    _pending = pending;
    // ignore() keeps a failPending/onRemoteClosed that lands while send() is
    // still awaited from becoming an unhandled error; `await response` below
    // still throws it.
    final response = pending.future.timeout(frameTimeout)..ignore();
    try {
      await send(body);
    } on Object {
      _pending = null;
      rethrow;
    }
    _completePending();
    try {
      return await response;
    } finally {
      _pending = null;
    }
  }

  Future<void> runInitiatorHandshake() async {
    final first = await _noise.writeMessage();
    final second = await exchange(first);
    await _noise.readMessage(second);
    final third = await _noise.writeMessage();
    await send(third);
    if (!_noise.isFinished) {
      throw StateError('The Noise handshake did not finish.');
    }
    messageSizes = <int>[first.length, second.length, third.length];
    status = NoiseHandshakeStatus.finished;
  }

  /// Encrypts [plaintext], waits for the encrypted reply and decrypts it.
  Future<Uint8List> exchangeEncrypted(Uint8List plaintext) async {
    final reply = await exchange(await _noise.encrypt(plaintext));
    return _noise.decrypt(reply);
  }

  /// Initiator side: [repetitions] encrypted round trips of [body]. Stops at
  /// the first failure (the Noise counters are desynced after it).
  Future<({List<double> timingsMilliseconds, Object? failure})>
  measureEncryptedEchoes({
    required Uint8List body,
    required int repetitions,
    required bool Function() shouldContinue,
  }) async {
    final timings = <double>[];
    for (var repetition = 0; repetition < repetitions; repetition++) {
      if (!shouldContinue()) {
        break;
      }
      final stopwatch = Stopwatch()..start();
      try {
        final echoed = await exchangeEncrypted(body);
        if (!listEquals(body, echoed)) {
          throw StateError('Eco incorreto no canal L2CAP.');
        }
        timings.add(stopwatch.elapsedMicroseconds / 1000);
      } on Object catch (failure) {
        return (timingsMilliseconds: timings, failure: failure);
      }
    }
    return (timingsMilliseconds: timings, failure: null);
  }

  /// Responder side: consumes msg1/msg3 and answers msg1 with msg2.
  Future<void> respondHandshake(Uint8List message) async {
    await _noise.readMessage(message);
    messageSizes = <int>[...messageSizes, message.length];
    if (_noise.isFinished) {
      status = NoiseHandshakeStatus.finished;
      return;
    }
    final reply = await _noise.writeMessage();
    messageSizes = <int>[...messageSizes, reply.length];
    await send(reply);
  }

  Future<Uint8List> decrypt(Uint8List ciphertext) => _noise.decrypt(ciphertext);

  /// Responder side: encrypts [plaintext] and sends it as one frame.
  Future<void> sendEncrypted(Uint8List plaintext) async =>
      send(await _noise.encrypt(plaintext));

  void markFailed() => status = NoiseHandshakeStatus.failed;

  void failPending(Object error) {
    final pending = _pending;
    _pending = null;
    if (pending != null && !pending.isCompleted) {
      pending.completeError(error);
    }
  }

  void onRemoteClosed() {
    isClosed = true;
    _failIfUnfinished();
    failPending(StateError('O canal L2CAP foi fechado.'));
  }

  Future<void> close() async {
    if (isClosed) {
      return;
    }
    onRemoteClosed();
    await _nativeService.closeL2cap(channelId: channelId);
  }

  void _failIfUnfinished() {
    if (status == NoiseHandshakeStatus.inProgress) {
      markFailed();
    }
  }

  void _completePending() {
    final pending = _pending;
    if (pending != null && !pending.isCompleted && _inbox.isNotEmpty) {
      _pending = null;
      pending.complete(_inbox.removeFirst());
    }
  }
}
