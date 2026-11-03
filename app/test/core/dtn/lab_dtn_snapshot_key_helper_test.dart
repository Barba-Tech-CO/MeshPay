import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/lab_dtn_snapshot_key_helper.dart';

void main() {
  test('the shared lab snapshot key is a compressed P-256 point', () {
    final key = LabDtnSnapshotKeyHelper.publicKey();

    expect(key, hasLength(33));
    expect(key[0], anyOf(2, 3));
    expect(LabDtnSnapshotKeyHelper.publicKey(), key);
  });
}
