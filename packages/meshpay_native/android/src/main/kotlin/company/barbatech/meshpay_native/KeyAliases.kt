package company.barbatech.meshpay_native

object KeyAliases {
  private const val prefix = "meshpay."
  private const val maxLength = 128
  private const val allowedSymbols = "._-"

  fun namespaced(alias: String): String {
    if (alias.isBlank() || alias.length > maxLength || !alias.all(::isAllowed)) {
      throw NativePluginException(
        "invalid_alias",
        "alias must be 1..$maxLength characters of A-Z a-z 0-9 . _ -",
      )
    }
    return prefix + alias
  }

  private fun isAllowed(char: Char): Boolean =
    char in 'a'..'z' || char in 'A'..'Z' || char in '0'..'9' || char in allowedSymbols
}
