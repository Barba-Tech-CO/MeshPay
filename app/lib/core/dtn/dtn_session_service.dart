import 'dart:typed_data';

/// Inbound side of one peer connection (Rust `DtnSession`): at most 20
/// envelope attempts per connection.
abstract interface class DtnSessionService {
  /// True when stored, false for a duplicate; throws
  /// [DtnRejectedException] for every other outcome.
  Future<bool> ingest(Uint8List envelope);
}
