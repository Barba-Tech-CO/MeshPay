package company.barbatech.meshpay_native

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KeyAliasesTest {
  @Test
  fun prefixesValidAliases() {
    assertEquals("meshpay.a-B_1.2", KeyAliases.namespaced("a-B_1.2"))
  }

  @Test
  fun rejectsBlankLongAndForeignCharacters() {
    listOf("", "  ", "a b", "a/b", "é", "x".repeat(129)).forEach {
      val error = assertFailsWith<NativePluginException> { KeyAliases.namespaced(it) }
      assertEquals("invalid_alias", error.code)
    }
  }
}
