import 'dart:async';
import 'dart:io';

import 'package:meshpay_native/clock_sample.dart';

import '../storage/app_support_directory_service.dart';
import 'clock_log_entry.dart';
import 'clock_log_repository.dart';

class JsonLinesClockLogRepository implements ClockLogRepository {
  JsonLinesClockLogRepository(this._directoryService);

  final AppSupportDirectoryService _directoryService;
  Future<void> _writeQueue = Future<void>.value();

  @override
  Future<List<ClockLogEntry>> readAll() async {
    final file = await _logFile();
    if (!await file.exists()) {
      return const <ClockLogEntry>[];
    }

    final lines = await file.readAsLines();
    return List<ClockLogEntry>.unmodifiable(
      lines
          .where(
            (line) {
              return line.isNotEmpty;
            },
          )
          .map(ClockLogEntry.fromJsonLine),
    );
  }

  @override
  Future<ClockLogEntry> append({
    required ClockSample sample,
    required String label,
    required bool isAppLaunch,
  }) {
    final completer = Completer<ClockLogEntry>();
    final operation = _writeQueue.then((_) async {
      try {
        final entries = await readAll();
        final entry = ClockLogEntry(
          sequence: entries.isEmpty ? 1 : entries.last.sequence + 1,
          elapsedRealtimeNanos: sample.elapsedRealtimeNanos,
          bootCount: sample.bootCount,
          label: label,
          isAppLaunch: isAppLaunch,
        );
        final file = await _logFile();
        await file.parent.create(recursive: true);
        await file.writeAsString(
          '${entry.toJsonLine()}\n',
          mode: FileMode.append,
          flush: true,
        );
        completer.complete(entry);
      } on Object catch (error, stackTrace) {
        completer.completeError(error, stackTrace);
      }
    });
    _writeQueue = operation;
    return completer.future;
  }

  Future<File> _logFile() async {
    final directory = await _directoryService.getPath();
    return File('$directory${Platform.pathSeparator}clock_log.jsonl');
  }
}
