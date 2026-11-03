package company.barbatech.meshpay_native

import android.security.keystore.StrongBoxUnavailableException

object StrongBoxGuard {
  // Single attempt: a StrongBox failure is surfaced as a refusal, never retried without StrongBox (PV-SEC-02).
  fun <T> generate(
    requireStrongBox: Boolean,
    isStrongBoxUnavailable: (Throwable) -> Boolean = { it is StrongBoxUnavailableException },
    generator: () -> T,
  ): T {
    try {
      return generator()
    } catch (error: Throwable) {
      if (requireStrongBox && isStrongBoxUnavailable(error)) {
        throw NativePluginException(
          "strongbox_unavailable",
          "StrongBox was requested but key generation failed there; no TEE fallback.",
        )
      }
      throw error
    }
  }
}
