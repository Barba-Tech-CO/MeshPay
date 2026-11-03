import 'native_payload.dart';

class DeviceCapabilities {
  const DeviceCapabilities({
    required this.hasStrongBox,
    required this.apiLevel,
    required this.supportsMaxUsageCount,
    required this.hasSingleUseKeyFeature,
    required this.hasLimitedUseKeyFeature,
  });

  final bool hasStrongBox;
  final int apiLevel;

  /// True on API 31+, where KeyGenParameterSpec.setMaxUsageCount exists.
  final bool supportsMaxUsageCount;
  final bool hasSingleUseKeyFeature;
  final bool hasLimitedUseKeyFeature;

  factory DeviceCapabilities.fromPlatform(Object? value) {
    final payload = NativePayload.readMap(value, 'device capabilities');
    return DeviceCapabilities(
      hasStrongBox: NativePayload.readBool(payload, 'hasStrongBox'),
      apiLevel: NativePayload.readIntField(payload, 'apiLevel'),
      supportsMaxUsageCount: NativePayload.readBool(
        payload,
        'supportsMaxUsageCount',
      ),
      hasSingleUseKeyFeature: NativePayload.readBool(
        payload,
        'hasSingleUseKeyFeature',
      ),
      hasLimitedUseKeyFeature: NativePayload.readBool(
        payload,
        'hasLimitedUseKeyFeature',
      ),
    );
  }
}
