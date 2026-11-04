import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../ble/native_lab_service.dart';
import '../dtn/dtn_buffer_service.dart';
import '../dtn/snapshot_config_helper.dart';
import '../keys/payment_key_identity.dart';
import '../keys/payment_key_signer.dart';
import '../server/trust/server_trust_service.dart';
import '../storage/app_support_directory_service.dart';
import 'external_tx_signing_helper.dart';
import 'issuer_config_helper.dart';
import 'lab_cheque_stub_helper.dart';
import 'lab_tx_storage_helper.dart';
import 'receiver_clock_helper.dart';
import 'receiver_tx_engine_service.dart';

/// Receiver engine whose acks are signed by the Keystore key `lab_receiver`.
class MeshpayCoreExternalReceiverTxEngineService
    implements ReceiverTxEngineService {
  MeshpayCoreExternalReceiverTxEngineService(
    this._directory,
    this._signer,
    this._dtn,
    this._native,
    this._trust,
  );

  static const String _alias = 'lab_receiver';
  static const String _side = 'receiver_external';

  final AppSupportDirectoryService _directory;
  final PaymentKeySigner _signer;
  final DtnBufferService _dtn;
  final NativeLabService _native;
  final ServerTrustService _trust;
  ReceiverTxEngine? _engine;

  @override
  Future<Uint8List> getPublicKey() async => (await _ensureKey()).publicKey;

  @override
  Future<void> open({required Uint8List payerPublicKey}) async {
    await close();
    final identity = await _ensureKey();
    final keys = await LabTxStorageHelper.loadOrCreateKeys(_directory, _side);
    _engine = await ReceiverTxEngine.openExternal(
      dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
      dbKey: keys.dbKey,
      ownPublicKey: identity.publicKey,
      payerPublicKey: payerPublicKey,
      ownAccountId: identity.publicKey,
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
    return ExternalTxSigningHelper.acknowledge(
      engine: engine,
      signer: _signer,
      alias: _alias,
      propose: propose,
      clock: await ReceiverClockHelper.prepare(
        engine: engine,
        dtn: _dtn,
        native: _native,
        trust: _trust,
      ),
    );
  }

  @override
  Future<void> close() async {
    _engine?.dispose();
    _engine = null;
  }

  Future<PaymentKeyIdentity> _ensureKey() async => _signer.ensureKey(
    _alias,
    dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
  );
}
