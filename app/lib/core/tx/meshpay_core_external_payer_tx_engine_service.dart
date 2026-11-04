import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../keys/payment_key_identity.dart';
import '../keys/payment_key_signer.dart';
import '../storage/app_support_directory_service.dart';
import 'external_tx_signing_helper.dart';
import 'lab_tx_storage_helper.dart';
import 'payer_tx_engine_service.dart';

/// Payer engine whose cheques are signed by the Keystore key `lab_payer`.
class MeshpayCoreExternalPayerTxEngineService implements PayerTxEngineService {
  MeshpayCoreExternalPayerTxEngineService(this._directory, this._signer);

  static const String _alias = 'lab_payer';
  static const String _side = 'payer_external';

  final AppSupportDirectoryService _directory;
  final PaymentKeySigner _signer;
  PayerTxEngine? _engine;
  Future<void> _counterQueue = Future<void>.value();

  @override
  Future<Uint8List> getPublicKey() async => (await _ensureKey()).publicKey;

  @override
  Future<int> nextCounter() {
    final next = _counterQueue.then(
      (_) => LabTxStorageHelper.nextCounter(_directory, _side),
    );
    _counterQueue = next.then<void>((_) {}, onError: (Object _) {});
    return next;
  }

  @override
  Future<void> open({required Uint8List payeePublicKey}) async {
    await close();
    final identity = await _ensureKey();
    final keys = await LabTxStorageHelper.loadOrCreateKeys(_directory, _side);
    _engine = await PayerTxEngine.openExternal(
      dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
      dbKey: keys.dbKey,
      ownPublicKey: identity.publicKey,
      payeePublicKey: payeePublicKey,
    );
  }

  @override
  Future<Uint8List> propose({
    required Uint8List spendBody,
    required int counter,
    required Uint8List recipientId,
    required Uint8List stub,
  }) => ExternalTxSigningHelper.propose(
    engine: _opened,
    signer: _signer,
    alias: _alias,
    spendBody: spendBody,
    counter: BigInt.from(counter),
    recipientId: recipientId,
    stub: stub,
  );

  @override
  Future<void> onAck(Uint8List ack) => _opened.onAck(ack: ack);

  @override
  Future<List<Uint8List>> resumePending() => _opened.resumePending();

  @override
  Future<void> close() async {
    _engine?.dispose();
    _engine = null;
  }

  Future<PaymentKeyIdentity> _ensureKey() async => _signer.ensureKey(
    _alias,
    dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
  );

  PayerTxEngine get _opened =>
      _engine ?? (throw StateError('tx engine not open'));
}
