import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../ble/native_lab_service.dart';
import '../dtn/dtn_buffer_service.dart';
import '../dtn/snapshot_config_helper.dart';
import '../server/trust/server_trust_service.dart';
import '../storage/app_support_directory_service.dart';
import 'issuer_config_helper.dart';
import 'lab_cheque_stub_helper.dart';
import 'lab_tx_storage_helper.dart';
import 'receiver_clock_helper.dart';
import 'receiver_tx_engine_service.dart';

class MeshpayCoreReceiverTxEngineService implements ReceiverTxEngineService {
  MeshpayCoreReceiverTxEngineService(
    this._directory,
    this._dtn,
    this._native,
    this._trust,
  );

  static const String _side = 'receiver';

  final AppSupportDirectoryService _directory;
  final DtnBufferService _dtn;
  final NativeLabService _native;
  final ServerTrustService _trust;
  ReceiverTxEngine? _engine;

  @override
  Future<Uint8List> getPublicKey() =>
      LabTxStorageHelper.publicKey(_directory, _side);

  @override
  Future<void> open({required Uint8List payerPublicKey}) async {
    await close();
    final keys = await LabTxStorageHelper.loadOrCreateKeys(_directory, _side);
    _engine = await ReceiverTxEngine.open(
      dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
      dbKey: keys.dbKey,
      signingSeed: keys.seed,
      payerPublicKey: payerPublicKey,
      ownAccountId: keys.publicKey,
      snapshotCert: await SnapshotConfigHelper.cert(
        fetch: _trust.snapshotCert,
      ),
      expirySecs: BigInt.from(LabChequeStubHelper.expirySeconds),
      issuerRootPublicKey: await IssuerConfigHelper.rootPublicKey(),
      issuerCert: await IssuerConfigHelper.cert(fetch: _trust.issuerCert),
    );
  }

  @override
  Future<Uint8List> onPropose(Uint8List propose) async {
    final engine = _engine ?? (throw StateError('tx engine not open'));
    final sample = await ReceiverClockHelper.prepare(
      engine: engine,
      dtn: _dtn,
      native: _native,
      trust: _trust,
    );
    return engine.onPropose(
      propose: propose,
      monotonicNanos: BigInt.from(sample.elapsedRealtimeNanos),
      bootCount: ReceiverClockHelper.bootCount(sample),
    );
  }

  @override
  Future<void> close() async {
    _engine?.dispose();
    _engine = null;
  }
}
