import 'l2cap_closed_event.dart';
import 'l2cap_data_event.dart';
import 'l2cap_opened_event.dart';
import 'native_error_event.dart';
import 'native_event.dart';
import 'native_payload.dart';
import 'native_protocol_exception.dart';
import 'scan_result_event.dart';

class NativeEventDecoder {
  static NativeEvent decode(Object? value) {
    final payload = NativePayload.readMap(value, 'native event');
    return switch (NativePayload.readString(payload, 'type')) {
      'scanResult' => ScanResultEvent(
        deviceId: NativePayload.readString(payload, 'deviceId'),
        rssi: NativePayload.readIntField(payload, 'rssi'),
        sessionIdHex: NativePayload.readString(payload, 'sessionIdHex'),
        elapsedRealtimeNanos: NativePayload.readIntField(
          payload,
          'elapsedRealtimeNanos',
        ),
      ),
      'l2capOpened' => L2capOpenedEvent(
        channelId: NativePayload.readIntField(payload, 'channelId'),
        role: NativePayload.readString(payload, 'role'),
        deviceId: NativePayload.readString(payload, 'deviceId'),
      ),
      'l2capData' => L2capDataEvent(
        channelId: NativePayload.readIntField(payload, 'channelId'),
        data: NativePayload.readBytes(payload['data'], 'data'),
      ),
      'l2capClosed' => L2capClosedEvent(
        channelId: NativePayload.readIntField(payload, 'channelId'),
      ),
      'error' => NativeErrorEvent(
        code: NativePayload.readString(payload, 'code'),
        message: NativePayload.readString(payload, 'message'),
        channelId: NativePayload.readOptionalInt(payload, 'channelId'),
      ),
      final type => throw NativeProtocolException(
        'Unknown native event type: $type.',
      ),
    };
  }
}
