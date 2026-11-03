package company.barbatech.meshpay_native

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class StrongBoxGuardTest {
  private class FakeStrongBoxUnavailable : RuntimeException()

  private val isFake: (Throwable) -> Boolean = { it is FakeStrongBoxUnavailable }

  @Test
  fun strongBoxFailureIsRefusedNotDowngraded() {
    var attempts = 0
    val error = assertFailsWith<NativePluginException> {
      StrongBoxGuard.generate(true, isFake) {
        attempts++
        throw FakeStrongBoxUnavailable()
      }
    }
    assertEquals("strongbox_unavailable", error.code)
    assertEquals(1, attempts)
  }

  @Test
  fun everyRefusedLevelStaysRefused() {
    for (level in listOf("tee", "software", "unknown")) {
      var attempts = 0
      val error = assertFailsWith<NativePluginException>("level $level") {
        StrongBoxGuard.generate(true, isFake) {
          attempts++
          throw FakeStrongBoxUnavailable()
        }
      }
      assertEquals("strongbox_unavailable", error.code, level)
      assertEquals(1, attempts, level)
    }
  }

  @Test
  fun unrelatedErrorsPassThrough() {
    assertFailsWith<IllegalStateException> {
      StrongBoxGuard.generate(true, isFake) { throw IllegalStateException("x") }
    }
  }
}
