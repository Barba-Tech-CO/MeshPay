package company.barbatech.meshpay_native

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class SessionIdParserTest {
  @Test
  fun parsesOneToEightBytesOfHexadecimal() {
    assertContentEquals(
      byteArrayOf(0x01, 0xab.toByte(), 0xff.toByte()),
      SessionIdParser.parse("01aBff"),
    )
    assertEquals(8, SessionIdParser.parse("0011223344556677").size)
  }

  @Test
  fun rejectsInvalidLengthsAndCharactersWithTypedErrors() {
    val tooLong = assertFailsWith<NativePluginException> {
      SessionIdParser.parse("001122334455667788")
    }
    val malformed = assertFailsWith<NativePluginException> {
      SessionIdParser.parse("0g")
    }
    val oddLength = assertFailsWith<NativePluginException> {
      SessionIdParser.parse("abc")
    }

    assertEquals("invalid_session_id", tooLong.code)
    assertEquals("invalid_session_id", malformed.code)
    assertEquals("invalid_session_id", oddLength.code)
  }
}
