package company.barbatech.meshpay_native

object SessionIdParser {
  fun parse(sessionIdHex: String): ByteArray {
    if (sessionIdHex.length !in 2..16 || sessionIdHex.length % 2 != 0) {
      throw NativePluginException(
        "invalid_session_id",
        "Session identifier must contain 1 to 8 bytes of hexadecimal data.",
      )
    }

    return ByteArray(sessionIdHex.length / 2) { index ->
      val high = hexDigit(sessionIdHex[index * 2])
      val low = hexDigit(sessionIdHex[index * 2 + 1])
      if (high < 0 || low < 0) {
        throw NativePluginException(
          "invalid_session_id",
          "Session identifier must contain only hexadecimal characters.",
        )
      }
      ((high shl 4) or low).toByte()
    }
  }

  private fun hexDigit(character: Char): Int =
    when (character) {
      in '0'..'9' -> character - '0'
      in 'a'..'f' -> character - 'a' + 10
      in 'A'..'F' -> character - 'A' + 10
      else -> -1
    }
}
