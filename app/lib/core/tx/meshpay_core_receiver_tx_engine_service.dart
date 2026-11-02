import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import '../storage/app_support_directory_service.dart';
import 'lab_tx_storage_helper.dart';
import 'receiver_tx_engine_service.dart';

class MeshpayCoreReceiverTxEngineService implements ReceiverTxEngineService {
  MeshpayCoreReceiverTxEngineService(this._directory);

  static const String _side = 'receiver';

  final AppSupportDirectoryService _directory;
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
    );
  }

  @override
  Future<Uint8List> onPropose(Uint8List propose) async {
    final engine = _engine ?? (throw StateError('tx engine not open'));
    return engine.onPropose(propose: propose);
  }

  @override
  Future<void> close() async {
    _engine?.dispose();
    _engine = null;
  }
}
