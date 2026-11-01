package company.barbatech.meshpay_native

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class LoopbackPayloadValidatorTest {
  @Test
  fun acceptsOnlyAnIdenticalNonNullPayload() {
    val payload = byteArrayOf(1, 2, 3)

    assertTrue(LoopbackPayloadValidator.matches(payload, payload.copyOf()))
    assertFalse(LoopbackPayloadValidator.matches(payload, byteArrayOf(1, 2)))
    assertFalse(LoopbackPayloadValidator.matches(payload, null))
  }
}
