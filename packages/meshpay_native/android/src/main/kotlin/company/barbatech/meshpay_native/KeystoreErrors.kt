package company.barbatech.meshpay_native

import android.security.keystore.StrongBoxUnavailableException
import java.security.InvalidKeyException
import java.security.SignatureException

object KeystoreErrors {
  fun map(error: Throwable): NativePluginException = when (error) {
    is NativePluginException -> error
    is StrongBoxUnavailableException -> NativePluginException(
      "strongbox_unavailable",
      "StrongBox was requested but is not available on this device.",
    )
    is InvalidKeyException -> NativePluginException(
      "key_unusable",
      "The key is missing, exhausted or permanently invalidated.",
    )
    is SignatureException -> NativePluginException(
      "sign_failed",
      "The signature operation failed.",
    )
    else -> NativePluginException(
      "keystore_error",
      error.message ?: error.javaClass.simpleName,
    )
  }
}
