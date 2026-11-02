import 'tx_error_code.dart';

/// A lab protocol failure with a fixed [code]; [isRemote] when the peer
/// refused with that code.
class TxLabException implements Exception {
  const TxLabException(this.code, {this.isRemote = false});

  final TxErrorCode code;
  final bool isRemote;
}
