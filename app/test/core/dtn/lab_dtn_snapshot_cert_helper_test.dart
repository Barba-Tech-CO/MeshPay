import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/lab_dtn_snapshot_cert_helper.dart';
import 'package:meshpay_core/meshpay_core.dart'
    show RustLib, labIssuerRootPublicKey, verifySnapshotCertAtTime;

void main() {
  setUpAll(() {
    final libraryName = switch (Platform.operatingSystem) {
      'macos' => 'libmeshpay_core.dylib',
      'windows' => 'meshpay_core.dll',
      _ => 'libmeshpay_core.so',
    };
    return RustLib.init(
      externalLibrary: ExternalLibrary.open(
        File('../core/target/debug/$libraryName').absolute.path,
      ),
    );
  });

  test(
    'lab cert passes the core window check for old and new snapshots',
    () async {
      final cert = await LabDtnSnapshotCertHelper.cert();
      final rootPublicKey = await labIssuerRootPublicKey();
      final nowUnix = DateTime.now().millisecondsSinceEpoch ~/ 1000;

      expect(cert, isNotEmpty);
      await verifySnapshotCertAtTime(
        rootPublicKey: rootPublicKey,
        cert: cert,
        timestampUnix: BigInt.from(
          nowUnix - LabDtnSnapshotCertHelper.certBackdateSeconds,
        ),
      );
      await verifySnapshotCertAtTime(
        rootPublicKey: rootPublicKey,
        cert: cert,
        timestampUnix: BigInt.from(nowUnix),
      );
    },
  );
}
