import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../storage/app_support_directory_service.dart';
import 'lab_cheque_stub_helper.dart';
import 'lab_tx_storage_helper.dart';
import 'payer_tx_engine_service.dart';

class MeshpayCorePayerTxEngineService implements PayerTxEngineService {
  MeshpayCorePayerTxEngineService(this._directory);

  static const String _side = 'payer';

  final AppSupportDirectoryService _directory;
  PayerTxEngine? _engine;
  Future<void> _counterQueue = Future<void>.value();

  @override
  Future<Uint8List> getPublicKey() =>
      LabTxStorageHelper.publicKey(_directory, _side);

  @override
  Future<Uint8List> labStub({
    required Uint8List chequeId,
    required int maxAmountUnits,
    required int issuedAtUnix,
  }) async => LabChequeStubHelper.forCheque(
    chequeId: chequeId,
    signerPubkey: await getPublicKey(),
    maxAmountUnits: maxAmountUnits,
    issuedAtUnix: issuedAtUnix,
  );

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
    final keys = await LabTxStorageHelper.loadOrCreateKeys(_directory, _side);
    _engine = await PayerTxEngine.open(
      dbPath: await LabTxStorageHelper.dbPath(_directory, _side),
      dbKey: keys.dbKey,
      signingSeed: keys.seed,
      payeePublicKey: payeePublicKey,
    );
  }

  @override
  Future<Uint8List> propose({
    required Uint8List spendBody,
    required int counter,
    required Uint8List recipientId,
    required Uint8List stub,
  }) => _opened.propose(
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

  PayerTxEngine get _opened =>
      _engine ?? (throw StateError('tx engine not open'));
}
