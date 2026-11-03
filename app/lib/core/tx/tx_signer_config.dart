/// LAB switch: true signs cheques with the Android Keystore (external mode);
/// false keeps the software engines with plaintext seeds as lab fallback.
/// Non-Android platforms always use the software engines.
class TxSignerConfig {
  TxSignerConfig._();

  static const bool useKeystoreOnAndroid = true;
}
