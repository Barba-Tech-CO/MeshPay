import 'native_event.dart';

class L2capOpenedEvent extends NativeEvent {
  const L2capOpenedEvent({
    required this.channelId,
    required this.role,
    required this.deviceId,
  });

  final int channelId;
  final String role;
  final String deviceId;
}
