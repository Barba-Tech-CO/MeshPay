package company.barbatech.meshpay_native

import java.util.concurrent.atomic.AtomicInteger

class L2capConnectionLimiter(
  private val maxConnections: Int,
) {
  private val connectionCount = AtomicInteger()

  fun tryAcquire(): Boolean {
    while (true) {
      val currentCount = connectionCount.get()
      if (currentCount >= maxConnections) {
        return false
      }
      if (connectionCount.compareAndSet(currentCount, currentCount + 1)) {
        return true
      }
    }
  }

  fun release() {
    while (true) {
      val currentCount = connectionCount.get()
      if (currentCount == 0) {
        return
      }
      if (connectionCount.compareAndSet(currentCount, currentCount - 1)) {
        return
      }
    }
  }

  companion object {
    const val maxServerConnections = 4
  }
}
