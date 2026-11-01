package company.barbatech.meshpay_native

object LoopbackPayloadValidator {
  fun matches(expected: ByteArray, actual: ByteArray?): Boolean =
    actual != null && expected.contentEquals(actual)
}
