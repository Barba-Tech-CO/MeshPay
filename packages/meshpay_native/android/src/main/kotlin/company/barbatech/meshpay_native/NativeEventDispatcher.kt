package company.barbatech.meshpay_native

import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.EventChannel

class NativeEventDispatcher : EventChannel.StreamHandler {
  private val mainHandler = Handler(Looper.getMainLooper())

  @Volatile
  private var eventSink: EventChannel.EventSink? = null

  override fun onListen(arguments: Any?, events: EventChannel.EventSink) {
    eventSink = events
  }

  override fun onCancel(arguments: Any?) {
    eventSink = null
  }

  fun emit(event: Map<String, Any?>) {
    mainHandler.post {
      eventSink?.success(event)
    }
  }

  fun emitError(
    code: String,
    message: String,
    channelId: Int? = null,
  ) {
    emit(
      mapOf(
        "type" to "error",
        "code" to code,
        "message" to message,
        "channelId" to channelId,
      ),
    )
  }
}
