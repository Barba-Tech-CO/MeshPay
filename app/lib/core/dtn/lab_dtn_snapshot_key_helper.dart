import 'dart:typed_data';

import '../noise/hex_helper.dart';

/// LAB ONLY: the SNAPSHOT role public key shared by every lab device, so two
/// devices accept the same signed snapshots. It is the compressed P-256 public
/// key of the private scalar SHA-256("meshpay-lab-snapshot-key-v1")
/// (0x204b19a3055eabd55add2d7f4ef7798409db2d13c94fe245d6f60deba0db75d7), which
/// is public on purpose: anyone can sign lab snapshots. A real build pins the
/// operator's key.
class LabDtnSnapshotKeyHelper {
  LabDtnSnapshotKeyHelper._();

  static const String _publicKeyHex =
      '020ead5d36dff23dbd62afe6443ded710a8b30b8f0a208c24c2597fe99e128ab33';

  static Uint8List publicKey() => HexHelper.decode(_publicKeyHex);
}
