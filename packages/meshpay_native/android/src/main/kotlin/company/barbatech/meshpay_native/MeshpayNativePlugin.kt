package company.barbatech.meshpay_native

import android.content.Context
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.MethodChannel.MethodCallHandler
import java.io.IOException

class MeshpayNativePlugin : FlutterPlugin, MethodCallHandler {
  private lateinit var methodChannel: MethodChannel
  private lateinit var eventChannel: EventChannel
  private lateinit var clockReader: ClockReader
  private lateinit var eventDispatcher: NativeEventDispatcher
  private lateinit var bluetoothController: BluetoothLeController
  private lateinit var l2capManager: L2capManager
  private lateinit var loopbackEcho: LoopbackEcho

  override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
    val context: Context = binding.applicationContext
    methodChannel = MethodChannel(
      binding.binaryMessenger,
      "meshpay_native/methods",
    )
    eventChannel = EventChannel(
      binding.binaryMessenger,
      "meshpay_native/events",
    )
    eventDispatcher = NativeEventDispatcher()
    clockReader = ClockReader(context)
    bluetoothController = BluetoothLeController(context, eventDispatcher)
    l2capManager = L2capManager(context, eventDispatcher)
    loopbackEcho = LoopbackEcho(methodChannel)
    methodChannel.setMethodCallHandler(this)
    eventChannel.setStreamHandler(eventDispatcher)
  }

  override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
    try {
      when (call.method) {
        "readClock" -> result.success(clockReader.readClock())
        "getBluetoothStatus" -> result.success(bluetoothController.getStatus())
        "startAdvertising" -> startAdvertising(call, result)
        "stopAdvertising" -> {
          bluetoothController.stopAdvertising()
          result.success(null)
        }
        "startScan" -> {
          bluetoothController.startScan(call.optionalString("sessionIdHex"))
          result.success(null)
        }
        "stopScan" -> {
          bluetoothController.stopScan()
          result.success(null)
        }
        "listenL2cap" -> result.success(l2capManager.listen())
        "stopListenL2cap" -> {
          l2capManager.stopListening()
          result.success(null)
        }
        "openL2cap" -> openL2cap(call, result)
        "sendL2cap" -> sendL2cap(call, result)
        "closeL2cap" -> {
          l2capManager.close(call.requiredInt("channelId"))
          result.success(null)
        }
        "runLoopbackEcho" -> loopbackEcho.run(
          call.requiredInt("sizeBytes"),
          call.requiredInt("iterations"),
          result,
        )
        else -> result.notImplemented()
      }
    } catch (error: NativePluginException) {
      result.error(error.code, error.message, null)
    } catch (error: SecurityException) {
      result.error(
        "permission_denied",
        "Bluetooth permission is missing or was revoked.",
        null,
      )
    } catch (error: IOException) {
      result.error("bluetooth_io_error", "The Bluetooth operation failed.", null)
    } catch (error: IllegalArgumentException) {
      result.error("invalid_argument", "A Bluetooth argument is invalid.", null)
    }
  }

  override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
    methodChannel.setMethodCallHandler(null)
    eventChannel.setStreamHandler(null)
    bluetoothController.dispose()
    l2capManager.dispose()
  }

  private fun startAdvertising(
    call: MethodCall,
    result: MethodChannel.Result,
  ) {
    bluetoothController.startAdvertising(
      call.requiredString("sessionIdHex"),
    ) { error ->
      if (error == null) {
        result.success(null)
      } else {
        result.error(error.code, error.message, null)
      }
    }
  }

  private fun openL2cap(call: MethodCall, result: MethodChannel.Result) {
    l2capManager.openClient(
      call.requiredString("deviceId"),
      call.requiredInt("psm"),
    ) { channelId, error ->
      if (error == null) {
        result.success(channelId)
      } else {
        result.error(error.code, error.message, null)
      }
    }
  }

  private fun sendL2cap(call: MethodCall, result: MethodChannel.Result) {
    l2capManager.send(
      call.requiredInt("channelId"),
      call.requiredBytes("data").copyOf(),
    ) { error ->
      if (error == null) {
        result.success(null)
      } else {
        result.error(error.code, error.message, null)
      }
    }
  }

  private fun MethodCall.requiredString(name: String): String =
    argumentsMap()[name] as? String
      ?: throw NativePluginException(
        "invalid_argument",
        "$name must be a string.",
      )

  private fun MethodCall.optionalString(name: String): String? =
    when (val value = argumentsMap()[name]) {
      null -> null
      is String -> value
      else -> throw NativePluginException(
        "invalid_argument",
        "$name must be a string or null.",
      )
    }

  private fun MethodCall.requiredInt(name: String): Int {
    val value = argumentsMap()[name]
    return when (value) {
      is Int -> value
      is Long -> {
        if (value !in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
          throw NativePluginException(
            "invalid_argument",
            "$name is outside the supported integer range.",
          )
        }
        value.toInt()
      }
      else -> throw NativePluginException(
        "invalid_argument",
        "$name must be an integer.",
      )
    }
  }

  private fun MethodCall.requiredBytes(name: String): ByteArray =
    argumentsMap()[name] as? ByteArray
      ?: throw NativePluginException("invalid_data", "$name must be bytes.")

  private fun MethodCall.argumentsMap(): Map<*, *> {
    val value = arguments
    if (value is Map<*, *>) {
      return value
    }
    if (value == null) {
      return emptyMap<Any, Any>()
    }
    throw NativePluginException("invalid_arguments", "Method arguments must be a map.")
  }
}
