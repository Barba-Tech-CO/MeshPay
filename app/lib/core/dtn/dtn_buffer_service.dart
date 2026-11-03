import 'dart:typed_data';

import 'dtn_session_service.dart';

/// Third-party DTN envelope buffer (Rust `DtnBuffer`).
abstract interface class DtnBufferService {
  /// Held envelope ids (32 bytes each).
  Future<List<Uint8List>> summary();

  /// Envelopes the peer lacks, at most 20.
  Future<List<Uint8List>> missingFrom(List<Uint8List> peerSummary);

  /// One session per connected peer.
  /// [originPair] is the Noise peer pair key; the pair's daily cap applies.
  Future<DtnSessionService> openSession({required Uint8List originPair});

  /// Builds an envelope around [payload] (ttl 4 hops) and stores it locally.
  Future<void> addTestEnvelope(Uint8List payload);

  Future<Uint8List?> snapshotBytes();

  /// The stored version, or null when the snapshot was refused (invalid or
  /// not newer).
  Future<int?> ingestSnapshot(Uint8List snapshot);
}
