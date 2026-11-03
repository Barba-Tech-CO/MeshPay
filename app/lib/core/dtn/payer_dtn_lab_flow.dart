import 'dart:typed_data';

import 'dtn_buffer_service.dart';
import 'dtn_gossip_session.dart';
import 'dtn_gossip_stats.dart';

/// Payer side of the lab DTN gossip: test envelope creation and the sync over
/// one established Noise session. One operation at a time; a call made while
/// another runs is ignored. The gossip counters outlive the connection.
class PayerDtnLabFlow {
  PayerDtnLabFlow(DtnBufferService buffer, void Function() onChanged)
    : _gossip = DtnGossipSession(buffer, onChanged);

  final DtnGossipSession _gossip;
  bool _isRunning = false;

  DtnGossipStats get stats => _gossip.stats;

  String? get error => _gossip.error;

  void startConnection() => _gossip.startConnection();

  Future<void> refreshHeld() => _gossip.refreshHeld();

  Future<void> createTestEnvelope() => _guard(_gossip.createTestEnvelope, null);

  /// Returns true when the link was lost (caller must tear the session down).
  Future<bool> sync(Future<Uint8List> Function(Uint8List plaintext) exchange) =>
      _guard(() => _gossip.sync(exchange), false);

  Future<T> _guard<T>(Future<T> Function() action, T whenBusy) async {
    if (_isRunning) {
      return whenBusy;
    }
    _isRunning = true;
    try {
      return await action();
    } finally {
      _isRunning = false;
    }
  }
}
