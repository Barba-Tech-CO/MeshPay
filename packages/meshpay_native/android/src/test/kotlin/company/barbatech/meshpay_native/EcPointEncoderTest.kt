package company.barbatech.meshpay_native

import java.security.KeyPairGenerator
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import kotlin.test.Test
import kotlin.test.assertEquals

class EcPointEncoderTest {
  @Test
  fun encodesFixedLengthUncompressedPoints() {
    val generator = KeyPairGenerator.getInstance("EC")
    generator.initialize(ECGenParameterSpec("secp256r1"))
    repeat(50) {
      val key = generator.generateKeyPair().public as ECPublicKey
      val encoded = EcPointEncoder.toUncompressed(key)
      assertEquals(65, encoded.size)
      assertEquals(0x04.toByte(), encoded[0])
      assertEquals(key.w.affineX, java.math.BigInteger(1, encoded.copyOfRange(1, 33)))
      assertEquals(key.w.affineY, java.math.BigInteger(1, encoded.copyOfRange(33, 65)))
    }
  }
}
