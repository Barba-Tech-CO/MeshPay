import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/tx/lab_tx_storage_helper.dart';
import 'package:meshpay/core/tx/meshpay_core_payer_tx_engine_service.dart';

import '../../fakes/fake_app_support_directory_service.dart';

void main() {
  late Directory directory;
  late FakeAppSupportDirectoryService support;

  setUp(() {
    directory = Directory.systemTemp.createTempSync('meshpay_tx_');
    support = FakeAppSupportDirectoryService(directory.path);
  });

  tearDown(() => directory.deleteSync(recursive: true));

  test('the counter never repeats, even on concurrent requests', () async {
    final service = MeshpayCorePayerTxEngineService(support);

    final first = await Future.wait(<Future<int>>[
      service.nextCounter(),
      service.nextCounter(),
      service.nextCounter(),
    ]);
    final restarted = await MeshpayCorePayerTxEngineService(
      support,
    ).nextCounter();

    expect(first, <int>[1, 2, 3]);
    expect(restarted, 4);
    expect(
      File('${directory.path}/meshpay_lab_payer.counter.tmp').existsSync(),
      isFalse,
    );
  });

  test('a corrupted counter or key file throws instead of resetting', () async {
    File('${directory.path}/meshpay_lab_payer.counter').writeAsStringSync('x');
    File('${directory.path}/meshpay_lab_payer.keys').writeAsBytesSync([1, 2]);

    await expectLater(
      LabTxStorageHelper.nextCounter(support, 'payer'),
      throwsFormatException,
    );
    await expectLater(
      LabTxStorageHelper.loadOrCreateKeys(support, 'payer'),
      throwsFormatException,
    );
    expect(File('${directory.path}/meshpay_lab_payer.keys').lengthSync(), 2);
  });
}
