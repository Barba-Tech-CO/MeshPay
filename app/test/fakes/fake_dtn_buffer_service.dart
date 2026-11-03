import 'dart:typed_data';

import 'package:meshpay/core/dtn/dtn_buffer_service.dart';
import 'package:meshpay/core/dtn/dtn_session_service.dart';
import 'package:meshpay/core/noise/hex_helper.dart';
import 'package:meshpay/core/tx/random_bytes_helper.dart';

import 'fake_dtn_session_service.dart';

/// In-memory buffer: an envelope is its 32-byte id followed by the payload.
/// `missingFrom` returns at most 20 envelopes the peer summary lacks.
class FakeDtnBufferService implements DtnBufferService {
  static const int idBytes = 32;

  final Map<String, Uint8List> held = <String, Uint8List>{};
  int sessionBudget = 20;
  bool ignorePeerSummary = false;
  Object? addError;
  Uint8List? snapshot;
  int? snapshotVersion;
  final List<FakeDtnSessionService> sessions = <FakeDtnSessionService>[];

  /// True when [envelope] is new.
  bool store(Uint8List envelope) {
    final key = HexHelper.encode(envelope.sublist(0, idBytes));
    if (held.containsKey(key)) {
      return false;
    }
    held[key] = envelope;
    return true;
  }

  @override
  Future<List<Uint8List>> summary() async => <Uint8List>[
    for (final envelope in held.values) envelope.sublist(0, idBytes),
  ];

  @override
  Future<List<Uint8List>> missingFrom(List<Uint8List> peerSummary) async {
    final known = <String>{
      if (!ignorePeerSummary) ...peerSummary.map(HexHelper.encode),
    };
    return held.entries
        .where((entry) => !known.contains(entry.key))
        .map((entry) => entry.value)
        .take(20)
        .toList();
  }

  @override
  Future<DtnSessionService> openSession() async {
    final session = FakeDtnSessionService(this, sessionBudget);
    sessions.add(session);
    return session;
  }

  @override
  Future<void> addTestEnvelope(Uint8List payload) async {
    if (addError case final error?) {
      throw error;
    }
    store(
      Uint8List.fromList(<int>[
        ...RandomBytesHelper.secure(idBytes),
        ...payload,
      ]),
    );
  }

  @override
  Future<Uint8List?> snapshotBytes() async => snapshot;

  /// The first byte is the version; only a newer one is stored.
  @override
  Future<int?> ingestSnapshot(Uint8List bytes) async {
    final version = bytes[0];
    if (version <= (snapshotVersion ?? 0)) {
      return null;
    }
    snapshot = bytes;
    return snapshotVersion = version;
  }
}
