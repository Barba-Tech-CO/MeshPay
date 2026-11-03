import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

import 'dtn_error_message_helper.dart';
import 'dtn_rejected_exception.dart';
import 'dtn_session_service.dart';

class MeshpayCoreDtnSessionService implements DtnSessionService {
  MeshpayCoreDtnSessionService(this._session);

  final DtnSession _session;

  @override
  Future<bool> ingest(Uint8List envelope) async {
    try {
      return await _session.ingest(
        envelope: envelope,
        nowUnix: BigInt.from(DateTime.now().millisecondsSinceEpoch ~/ 1000),
      );
    } on Object catch (failure) {
      final text = failure is String ? failure : '$failure';
      final outcome = DtnErrorMessageHelper.outcomeOf(text);
      if (outcome == null) {
        rethrow;
      }
      throw DtnRejectedException(outcome);
    }
  }
}
