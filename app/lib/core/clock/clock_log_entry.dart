import 'dart:convert';

class ClockLogEntry {
  const ClockLogEntry({
    required this.sequence,
    required this.elapsedRealtimeNanos,
    required this.bootCount,
    required this.label,
    required this.isAppLaunch,
  });

  final int sequence;
  final int elapsedRealtimeNanos;
  final int? bootCount;
  final String label;
  final bool isAppLaunch;

  factory ClockLogEntry.fromJsonLine(String line) {
    final Object? decoded = jsonDecode(line);
    if (decoded is! Map<Object?, Object?>) {
      throw const FormatException('Clock log entry must be a JSON object.');
    }

    final sequence = decoded['sequence'];
    final elapsedRealtimeNanos = decoded['elapsedRealtimeNanos'];
    final bootCount = decoded['bootCount'];
    final label = decoded['label'];
    final isAppLaunch = decoded['isAppLaunch'];
    if (sequence is! int ||
        elapsedRealtimeNanos is! int ||
        (bootCount != null && bootCount is! int) ||
        label is! String ||
        isAppLaunch is! bool) {
      throw const FormatException('Clock log entry has invalid fields.');
    }

    return ClockLogEntry(
      sequence: sequence,
      elapsedRealtimeNanos: elapsedRealtimeNanos,
      bootCount: bootCount as int?,
      label: label,
      isAppLaunch: isAppLaunch,
    );
  }

  String toJsonLine() => jsonEncode(<String, Object?>{
    'sequence': sequence,
    'elapsedRealtimeNanos': elapsedRealtimeNanos,
    'bootCount': bootCount,
    'label': label,
    'isAppLaunch': isAppLaunch,
  });

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ClockLogEntry &&
          sequence == other.sequence &&
          elapsedRealtimeNanos == other.elapsedRealtimeNanos &&
          bootCount == other.bootCount &&
          label == other.label &&
          isAppLaunch == other.isAppLaunch;

  @override
  int get hashCode =>
      sequence.hashCode ^
      elapsedRealtimeNanos.hashCode ^
      bootCount.hashCode ^
      label.hashCode ^
      isAppLaunch.hashCode;
}
