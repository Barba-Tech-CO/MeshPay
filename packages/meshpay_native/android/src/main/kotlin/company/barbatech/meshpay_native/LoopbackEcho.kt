package company.barbatech.meshpay_native

import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.MethodChannel
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class LoopbackEcho(private val methodChannel: MethodChannel) {
  private val mainHandler = Handler(Looper.getMainLooper())

  fun run(
    sizeBytes: Int,
    iterations: Int,
    result: MethodChannel.Result,
  ) {
    if (sizeBytes < 0 || iterations <= 0) {
      throw NativePluginException(
        "invalid_echo_size",
        "Echo size must be non-negative and iterations must be positive.",
      )
    }
    Thread(
      { runIterations(sizeBytes, iterations, result) },
      "meshpay-loopback-echo",
    ).start()
  }

  private fun runIterations(
    sizeBytes: Int,
    iterations: Int,
    result: MethodChannel.Result,
  ) {
    try {
      val timings = List(iterations) {
        val payload = ByteArray(sizeBytes) { index -> index.toByte() }
        val startedAtNanos = System.nanoTime()
        val response = requestEcho(payload)
        if (!LoopbackPayloadValidator.matches(payload, response)) {
          throw NativePluginException(
            "echo_payload_mismatch",
            "The loopback echo returned a different payload.",
          )
        }
        System.nanoTime() - startedAtNanos
      }
      mainHandler.post { result.success(timings) }
    } catch (error: NativePluginException) {
      mainHandler.post { result.error(error.code, error.message, null) }
    } catch (error: InterruptedException) {
      Thread.currentThread().interrupt()
      mainHandler.post {
        result.error("echo_interrupted", "The loopback echo was interrupted.", null)
      }
    }
  }

  private fun requestEcho(payload: ByteArray): ByteArray {
    val latch = CountDownLatch(1)
    var response: ByteArray? = null
    var failure: NativePluginException? = null
    mainHandler.post {
      methodChannel.invokeMethod(
        "echo",
        payload,
        object : MethodChannel.Result {
          override fun success(value: Any?) {
            if (value is ByteArray) {
              response = value.copyOf()
            } else {
              failure = NativePluginException(
                "echo_invalid_response",
                "The Dart echo handler must return a byte array.",
              )
            }
            latch.countDown()
          }

          override fun error(
            errorCode: String,
            errorMessage: String?,
            errorDetails: Any?,
          ) {
            failure = NativePluginException(
              "echo_failed",
              "The Dart echo handler returned an error.",
            )
            latch.countDown()
          }

          override fun notImplemented() {
            failure = NativePluginException(
              "echo_handler_missing",
              "No Dart echo handler is registered.",
            )
            latch.countDown()
          }
        },
      )
    }
    if (!latch.await(30, TimeUnit.SECONDS)) {
      throw NativePluginException("echo_timeout", "The Dart echo handler timed out.")
    }
    failure?.let { throw it }
    return response ?: throw NativePluginException(
      "echo_invalid_response",
      "The Dart echo handler returned no payload.",
    )
  }
}
