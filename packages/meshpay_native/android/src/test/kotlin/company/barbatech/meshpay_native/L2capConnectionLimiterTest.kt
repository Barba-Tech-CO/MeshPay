package company.barbatech.meshpay_native

import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class L2capConnectionLimiterTest {
  @Test
  fun acquiresUpToMaximumThenRefuses() {
    val limiter = L2capConnectionLimiter(3)

    assertTrue(limiter.tryAcquire())
    assertTrue(limiter.tryAcquire())
    assertTrue(limiter.tryAcquire())
    assertFalse(limiter.tryAcquire())
  }

  @Test
  fun releaseFreesASlot() {
    val limiter = L2capConnectionLimiter(1)

    assertTrue(limiter.tryAcquire())
    limiter.release()

    assertTrue(limiter.tryAcquire())
  }

  @Test
  fun releaseBelowZeroKeepsLimiterAtZero() {
    val limiter = L2capConnectionLimiter(1)

    limiter.release()

    assertTrue(limiter.tryAcquire())
  }

  @Test
  fun concurrentAcquisitionsNeverExceedMaximum() {
    val limiter = L2capConnectionLimiter(3)
    val pool = Executors.newFixedThreadPool(6)
    val ready = CountDownLatch(6)
    val start = CountDownLatch(1)
    val done = CountDownLatch(6)
    val acquired = AtomicInteger()

    repeat(6) {
      pool.execute {
        ready.countDown()
        start.await()
        if (limiter.tryAcquire()) {
          acquired.incrementAndGet()
        }
        done.countDown()
      }
    }

    ready.await()
    start.countDown()
    done.await()
    pool.shutdownNow()

    assertEquals(3, acquired.get())
  }
}
