import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:meshpay_native/payment_key_exception.dart';

import '../../../../core/keys/hardware_key_lab_service.dart';
import '../../../../core/tx/random_bytes_helper.dart';
import '../../domain/entities/hardware_key_probe_result.dart';
import '../helpers/payment_key_message_helper.dart';

class HardwareKeyLabViewModel extends ChangeNotifier {
  HardwareKeyLabViewModel(this._service);

  static const String probeAlias = 'lab_probe';
  static const int _challengeBytes = 32;
  static const String _keyNotFound = 'key_not_found';

  final HardwareKeyLabService _service;
  bool requireStrongBox = false;
  bool isRunning = false;
  HardwareKeyProbeResult? result;
  String? error;
  bool _isDisposed = false;

  void setRequireStrongBox(bool value) {
    requireStrongBox = value;
    _notifyIfActive();
  }

  /// Generates the attested key, signs a test message and always deletes the
  /// key it created.
  Future<void> runProbe() async {
    if (isRunning) {
      return;
    }
    isRunning = true;
    error = null;
    result = null;
    _notifyIfActive();
    var isGenerated = false;
    try {
      final capabilities = await _service.getDeviceCapabilities();
      await _deleteStaleProbeKey();
      final key = await _service.generateAttestedKey(
        alias: probeAlias,
        challenge: RandomBytesHelper.secure(_challengeBytes),
        requireStrongBox: requireStrongBox,
      );
      isGenerated = true;
      final signature = await _service.sign(
        alias: probeAlias,
        data: Uint8List.fromList(utf8.encode('meshpay-hardware-key-test')),
      );
      result = HardwareKeyProbeResult(
        securityLevel: key.securityLevel,
        hasStrongBox: capabilities.hasStrongBox,
        apiLevel: capabilities.apiLevel,
        chainLength: key.certificateChain.length,
        publicKey: key.publicKey,
        signatureLength: signature.length,
      );
    } on Object catch (failure) {
      error = PaymentKeyMessageHelper.messageFor(failure);
    } finally {
      if (isGenerated) {
        await _deleteProbeKey();
      }
      isRunning = false;
      _notifyIfActive();
    }
  }

  Future<void> _deleteStaleProbeKey() async {
    try {
      await _service.deleteKey(alias: probeAlias);
    } on PaymentKeyException catch (failure) {
      if (failure.code != _keyNotFound) {
        rethrow;
      }
    }
  }

  Future<void> _deleteProbeKey() async {
    try {
      await _service.deleteKey(alias: probeAlias);
    } on Object catch (failure) {
      error ??= PaymentKeyMessageHelper.messageFor(failure);
    }
  }

  void _notifyIfActive() {
    if (!_isDisposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    super.dispose();
  }
}
