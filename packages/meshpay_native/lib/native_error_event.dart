import 'native_event.dart';

class NativeErrorEvent extends NativeEvent {
  const NativeErrorEvent({
    required this.code,
    required this.message,
    required this.channelId,
  });

  final String code;
  final String message;
  final int? channelId;
}
