import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/keys/meshpay_native_payment_key_signer.dart';
import 'package:meshpay/core/tx/tx_error_code.dart';
import 'package:meshpay/core/tx/tx_error_message_helper.dart';
import 'package:meshpay/core/tx/tx_lab_exception.dart';

import '../../fakes/fake_app_support_directory_service.dart';
import '../../fakes/fake_keystore_native.dart';

void main() {
  late Directory directory;
  late FakeKeystoreNative native;
  late MeshpayNativePaymentKeySigner signer;
  late File pubFile;
  late String dbPath;

  setUp(() {
    directory = Directory.systemTemp.createTempSync('signer_test');
    native = FakeKeystoreNative();
    signer = MeshpayNativePaymentKeySigner(
      native,
      FakeAppSupportDirectoryService(directory.path),
    );
    pubFile = File('${directory.path}/meshpay_key_lab_payer.pub');
    dbPath = '${directory.path}/meshpay_lab_payer_external.db';
  });

  tearDown(() => directory.deleteSync(recursive: true));

  Uint8List validKey(int fill) =>
      Uint8List.fromList(<int>[4, ...List<int>.filled(64, fill)]);

  Future<Uint8List> ensure() async =>
      (await signer.ensureKey('lab_payer', dbPath: dbPath)).publicKey;

  test('generates once, saves the key atomically and reuses it', () async {
    final first = await ensure();
    final second = await ensure();

    expect(second, first);
    expect(pubFile.readAsBytesSync(), first);
    expect(File('${pubFile.path}.tmp').existsSync(), isFalse);
    expect(native.generated, 1);
  });

  test('concurrent calls generate a single key', () async {
    final keys = await Future.wait(<Future<Uint8List>>[
      ensure(),
      ensure(),
      ensure(),
    ]);

    expect(native.generated, 1);
    expect(keys[1], keys[0]);
    expect(keys[2], keys[0]);
  });

  test('a missing .pub regenerates the key when no database exists', () async {
    native.keys['lab_payer'] = validKey(9);

    final key = await ensure();

    expect(native.calls, contains('delete:lab_payer'));
    expect(native.generated, 1);
    expect(pubFile.readAsBytesSync(), key);
  });

  test('an invalid .pub regenerates the key when no database exists', () async {
    native.keys['lab_payer'] = validKey(9);
    pubFile.writeAsBytesSync(<int>[1, 2, 3]);

    final key = await ensure();

    expect(native.generated, 1);
    expect(pubFile.readAsBytesSync(), key);
  });

  test('a stale .pub without a Keystore key is regenerated', () async {
    pubFile.writeAsBytesSync(validKey(7));

    final key = await ensure();

    expect(key, isNot(validKey(7)));
    expect(pubFile.readAsBytesSync(), key);
    expect(native.calls, isNot(contains('delete:lab_payer')));
  });

  test('refuses to change identity when the database exists', () async {
    native.keys['lab_payer'] = validKey(9);
    File(dbPath).writeAsBytesSync(<int>[0]);

    await expectLater(
      ensure(),
      throwsA(
        isA<TxLabException>().having(
          (failure) => failure.code,
          'code',
          TxErrorCode.localKeyInconsistent,
        ),
      ),
    );

    expect(native.generated, 0);
    expect(native.keys['lab_payer'], validKey(9));
    expect(
      TxErrorMessageHelper.messageFor(
        const TxLabException(TxErrorCode.localKeyInconsistent),
      ),
      startsWith('Chave local inconsistente'),
    );
  });

  test('a consistent key and .pub with a database is left alone', () async {
    final first = await ensure();
    File(dbPath).writeAsBytesSync(<int>[0]);

    expect(await ensure(), first);
    expect(native.generated, 1);
  });

  test('a failed call is not cached', () async {
    File(dbPath).writeAsBytesSync(<int>[0]);
    await expectLater(ensure(), throwsA(isA<TxLabException>()));

    File(dbPath).deleteSync();
    expect(await ensure(), hasLength(65));
  });
}
