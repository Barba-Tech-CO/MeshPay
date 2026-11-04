import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../server/trust/server_trust_service.dart';
import '../storage/app_support_directory_service.dart';
import '../tx/issuer_config_helper.dart';
import '../tx/lab_tx_storage_helper.dart';
import 'dtn_buffer_service.dart';
import 'dtn_session_service.dart';
import 'meshpay_core_dtn_session_service.dart';
import 'snapshot_config_helper.dart';

/// LAB ONLY: the buffer database and its SQLCipher key live under the app
/// support dir with the tx lab keys (PLAINTEXT key file, see
/// [LabTxStorageHelper]).
class MeshpayCoreDtnBufferService implements DtnBufferService {
  MeshpayCoreDtnBufferService(this._directory, this._trust);

  static const String _side = 'dtn';

  final AppSupportDirectoryService _directory;
  final ServerTrustService _trust;
  Future<DtnBuffer>? _opening;

  @override
  Future<List<Uint8List>> summary() async => (await _buffer).summary();

  @override
  Future<List<Uint8List>> missingFrom(List<Uint8List> peerSummary) async =>
      (await _buffer).missingFrom(peerSummary: peerSummary);

  @override
  Future<DtnSessionService> openSession({
    required Uint8List originPair,
  }) async => MeshpayCoreDtnSessionService(
    (await _buffer).openSession(originPair: originPair),
  );

  @override
  Future<void> addTestEnvelope(Uint8List payload) async {
    final envelope = buildDtnEnvelope(
      encryptedServerBatch: payload,
      createdAtUnix: BigInt.from(DateTime.now().millisecondsSinceEpoch ~/ 1000),
      ttlHops: 4,
      knownRevocationVersion: BigInt.zero,
    );
    await (await _buffer).ingestLocal(
      envelope: envelope,
      nowUnix: BigInt.from(DateTime.now().millisecondsSinceEpoch ~/ 1000),
    );
  }

  @override
  Future<Uint8List?> snapshotBytes() async => (await _buffer).snapshotBytes();

  @override
  Future<int?> ingestSnapshot(Uint8List snapshot) async {
    try {
      return (await (await _buffer).ingestSnapshot(
        snapshot: snapshot,
      )).toInt();
    } on Object {
      return null;
    }
  }

  Future<DtnBuffer> get _buffer => _opening ??= _open();

  Future<DtnBuffer> _open() async {
    final keys = await LabTxStorageHelper.loadOrCreateKeys(_directory, _side);
    return DtnBuffer.open(
      dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
      dbKey: keys.dbKey,
      snapshotCert: await SnapshotConfigHelper.cert(
        fetch: _trust.snapshotCert,
      ),
      issuerRootPublicKey: await IssuerConfigHelper.rootPublicKey(),
      maxIngestPerMinute: 60,
    );
  }
}
