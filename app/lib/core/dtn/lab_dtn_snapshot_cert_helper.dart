import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

/// LAB ONLY: root-signed SNAPSHOT cert generated from the public lab key material.
class LabDtnSnapshotCertHelper {
  LabDtnSnapshotCertHelper._();

  static const int certBackdateSeconds = 29 * 24 * 60 * 60;

  static Future<Uint8List> cert() {
    final nowUnix = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    return labSnapshotCert(
      notBeforeUnix: BigInt.from(nowUnix - certBackdateSeconds),
    );
  }
}
