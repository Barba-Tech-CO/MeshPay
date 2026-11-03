import 'native_payload.dart';

class SingleUseKeyBatch {
  const SingleUseKeyBatch({
    required this.aliases,
    required this.durationsMs,
    required this.maxUsageCountApplied,
  });

  final List<String> aliases;

  /// Wall time to generate each key, same order as [aliases].
  final List<int> durationsMs;

  /// False when not requested or unsupported (API below 31).
  final bool maxUsageCountApplied;

  factory SingleUseKeyBatch.fromPlatform(Object? value) {
    final payload = NativePayload.readMap(value, 'single-use key batch');
    return SingleUseKeyBatch(
      aliases: NativePayload.readStringList(payload['aliases'], 'aliases'),
      durationsMs: NativePayload.readIntList(
        payload['durationsMs'],
        'durationsMs',
      ),
      maxUsageCountApplied: NativePayload.readBool(
        payload,
        'maxUsageCountApplied',
      ),
    );
  }
}
