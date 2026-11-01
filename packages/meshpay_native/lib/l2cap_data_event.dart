import 'dart:typed_data';

import 'native_event.dart';

class L2capDataEvent extends NativeEvent {
  const L2capDataEvent({required this.channelId, required this.data});

  final int channelId;
  final Uint8List data;
}
