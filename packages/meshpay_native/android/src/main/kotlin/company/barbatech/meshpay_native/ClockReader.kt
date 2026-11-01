package company.barbatech.meshpay_native

import android.content.Context
import android.os.SystemClock
import android.provider.Settings

class ClockReader(private val context: Context) {
  fun readClock(): Map<String, Any?> {
    val bootCount = Settings.Global.getInt(
      context.contentResolver,
      "boot_count",
      -1,
    )
    return mapOf(
      "elapsedRealtimeNanos" to SystemClock.elapsedRealtimeNanos(),
      "bootCount" to bootCount.takeIf { it >= 0 },
    )
  }
}
