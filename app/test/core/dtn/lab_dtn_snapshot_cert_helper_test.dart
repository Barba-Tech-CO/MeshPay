import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/lab_dtn_snapshot_cert_helper.dart';
import 'package:meshpay_core/meshpay_core.dart'
    show RustLib, labIssuerRootPublicKey, verifySnapshotCertAtTime;

void main() {
  final libraryName = switch (Platform.operatingSystem) {
    'macos' => 'libmeshpay_core.dylib',
    'windows' => 'meshpay_core.dll',
    _ => 'libmeshpay_core.so',
  };
  final nativeLibraryFile = File('../core/target/debug/$libraryName').absolute;
  test(
    'lab cert passes the core window check for old and new snapshots',
    () async {
      await RustLib.init(
        externalLibrary: ExternalLibrary.open(nativeLibraryFile.path),
      );
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
    skip: nativeLibraryFile.existsSync()
        ? false
        : 'native lib not built: run cargo build in core/ to enable',
  );
}
