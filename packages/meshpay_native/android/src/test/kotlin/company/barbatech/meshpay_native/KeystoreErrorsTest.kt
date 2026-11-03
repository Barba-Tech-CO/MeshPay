package company.barbatech.meshpay_native

import java.security.InvalidKeyException
import java.security.SignatureException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertSame

class KeystoreErrorsTest {
  @Test
  fun mapsJcaFailuresToStableCodes() {
    assertEquals("key_unusable", KeystoreErrors.map(InvalidKeyException()).code)
    assertEquals("sign_failed", KeystoreErrors.map(SignatureException()).code)
    assertEquals("keystore_error", KeystoreErrors.map(IllegalStateException("x")).code)
  }

  @Test
  fun keepsPluginExceptionsUntouched() {
    val original = NativePluginException("strongbox_unavailable", "m")
    assertSame(original, KeystoreErrors.map(original))
  }
}
