import 'dart:typed_data';

import 'package:meshpay/core/dtn/dtn_rejected_exception.dart';
import 'package:meshpay/core/dtn/dtn_session_service.dart';

import 'fake_dtn_buffer_service.dart';

/// Mirrors the Rust session: every attempt counts, the first [budget] reach
/// the buffer, later ones are refused with `SessionBudgetExceeded`.
class FakeDtnSessionService implements DtnSessionService {
  FakeDtnSessionService(this._buffer, this.budget);

  final FakeDtnBufferService _buffer;
  final int budget;
  int attempts = 0;

  @override
  Future<bool> ingest(Uint8List envelope) async {
    if (attempts++ >= budget) {
      throw const DtnRejectedException('SessionBudgetExceeded');
    }
    if (envelope.length < FakeDtnBufferService.idBytes) {
      throw const DtnRejectedException('Malformed');
    }
    return _buffer.store(envelope);
  }
}
