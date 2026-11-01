import 'native_event.dart';

class L2capClosedEvent extends NativeEvent {
  const L2capClosedEvent({required this.channelId});

  final int channelId;
}
