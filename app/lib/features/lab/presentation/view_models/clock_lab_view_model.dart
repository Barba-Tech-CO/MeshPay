import 'package:flutter/foundation.dart';

import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/clock/clock_log_entry.dart';
import '../../../../core/clock/clock_log_repository.dart';
import '../helpers/lab_error_message_helper.dart';

class ClockLabViewModel extends ChangeNotifier {
  ClockLabViewModel(this._nativeService, this._repository);

  final NativeLabService _nativeService;
  final ClockLogRepository _repository;
  List<ClockLogEntry> entries = const <ClockLogEntry>[];
  bool isLoading = false;
  String? error;
  bool _isDisposed = false;

  ClockLogEntry? get latestEntry => entries.isEmpty ? null : entries.first;

  Future<void> initialize() =>
      _recordSample('Abertura do laboratório', loadExistingEntries: true);

  Future<void> readSample() => _recordSample('Amostra manual');

  Future<void> _recordSample(
    String label, {
    bool loadExistingEntries = false,
  }) async {
    if (isLoading) {
      return;
    }
    isLoading = true;
    error = null;
    _notifyIfActive();
    try {
      if (loadExistingEntries) {
        entries = List<ClockLogEntry>.unmodifiable(
          (await _repository.readAll()).reversed,
        );
      }
      final sample = await _nativeService.readClock();
      final entry = await _repository.append(
        sample: sample,
        label: label,
        isAppLaunch: false,
      );
      entries = List<ClockLogEntry>.unmodifiable(<ClockLogEntry>[
        entry,
        ...entries,
      ]);
    } on Object catch (failure) {
      error = LabErrorMessageHelper.messageFor(failure);
    } finally {
      isLoading = false;
      _notifyIfActive();
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
