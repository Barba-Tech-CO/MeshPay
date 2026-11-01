package company.barbatech.meshpay_native

class NativePluginException(
  val code: String,
  message: String,
) : Exception(message)
