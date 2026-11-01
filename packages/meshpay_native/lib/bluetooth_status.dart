import 'native_payload.dart';

class BluetoothStatus {
  const BluetoothStatus({
    required this.adapterEnabled,
    required this.permissionsGranted,
    required this.le2mPhySupported,
    required this.leCodedPhySupported,
    required this.leExtendedAdvertisingSupported,
    required this.multipleAdvertisementSupported,
    required this.sdkInt,
  });

  final bool adapterEnabled;
  final bool permissionsGranted;
  final bool le2mPhySupported;
  final bool leCodedPhySupported;
  final bool leExtendedAdvertisingSupported;
  final bool multipleAdvertisementSupported;
  final int sdkInt;

  factory BluetoothStatus.fromPlatform(Object? value) {
    final payload = NativePayload.readMap(value, 'Bluetooth status');
    return BluetoothStatus(
      adapterEnabled: NativePayload.readBool(payload, 'adapterEnabled'),
      permissionsGranted: NativePayload.readBool(payload, 'permissionsGranted'),
      le2mPhySupported: NativePayload.readBool(payload, 'le2mPhySupported'),
      leCodedPhySupported: NativePayload.readBool(
        payload,
        'leCodedPhySupported',
      ),
      leExtendedAdvertisingSupported: NativePayload.readBool(
        payload,
        'leExtendedAdvertisingSupported',
      ),
      multipleAdvertisementSupported: NativePayload.readBool(
        payload,
        'multipleAdvertisementSupported',
      ),
      sdkInt: NativePayload.readIntField(payload, 'sdkInt'),
    );
  }
}
