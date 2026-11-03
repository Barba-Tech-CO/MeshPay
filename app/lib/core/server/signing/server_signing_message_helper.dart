import 'dart:convert';
import 'dart:typed_data';

/// Domain-separated messages the server verifies
/// (server/src/crypto/signing_messages.rs).
class ServerSigningMessageHelper {
  ServerSigningMessageHelper._();

  static Uint8List register(Uint8List publicKey) =>
      _prefixed('meshpay/register/v1', publicKey);

  static Uint8List issue(Uint8List requestBodyBytes) =>
      _prefixed('meshpay/issue/v1', requestBodyBytes);

  static Uint8List spend(Uint8List spendBodyBytes) =>
      _prefixed('meshpay/spend/v1', spendBodyBytes);

  /// account_id || timestamp (u64 big endian).
  static Uint8List balance(Uint8List accountId, int timestampUnix) =>
      _prefixed('meshpay/balance/v1', <int>[
        ...accountId,
        ...bigEndianUint64(timestampUnix),
      ]);

  static Uint8List bigEndianUint64(int value) =>
      (ByteData(8)..setUint64(0, value)).buffer.asUint8List();

  static Uint8List _prefixed(String domain, List<int> payload) =>
      Uint8List.fromList(<int>[...utf8.encode(domain), ...payload]);
}
