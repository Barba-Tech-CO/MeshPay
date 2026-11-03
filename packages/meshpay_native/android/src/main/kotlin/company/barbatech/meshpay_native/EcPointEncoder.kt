package company.barbatech.meshpay_native

import java.security.interfaces.ECPublicKey

object EcPointEncoder {
  private const val coordinateBytes = 32

  fun toUncompressed(key: ECPublicKey): ByteArray {
    val out = ByteArray(1 + 2 * coordinateBytes)
    out[0] = 0x04
    copyCoordinate(key.w.affineX.toByteArray(), out, 1)
    copyCoordinate(key.w.affineY.toByteArray(), out, 1 + coordinateBytes)
    return out
  }

  private fun copyCoordinate(source: ByteArray, out: ByteArray, offset: Int) {
    // BigInteger.toByteArray may add a leading sign byte or drop leading zeros.
    val trimmed = if (source.size > coordinateBytes) {
      source.copyOfRange(source.size - coordinateBytes, source.size)
    } else {
      source
    }
    trimmed.copyInto(out, offset + coordinateBytes - trimmed.size)
  }
}
