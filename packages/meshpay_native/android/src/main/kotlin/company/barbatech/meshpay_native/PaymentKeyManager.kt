package company.barbatech.meshpay_native

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.SystemClock
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec

class PaymentKeyManager(context: Context) {
  private companion object {
    const val maxSignBytes = 64 * 1024
    const val maxBatchCount = 200
  }

  private val packageManager: PackageManager = context.packageManager

  fun generateAttestedKey(
    alias: String,
    challenge: ByteArray,
    requireStrongBox: Boolean,
    singleUse: Boolean,
  ): Map<String, Any?> {
    if (challenge.isEmpty()) {
      throw NativePluginException("invalid_argument", "challenge must not be empty.")
    }
    if (singleUse && !supportsMaxUsageCount()) {
      throw NativePluginException(
        "max_usage_count_unsupported",
        "setMaxUsageCount needs API 31 or newer.",
      )
    }
    val name = KeyAliases.namespaced(alias)
    ensureFree(name)
    val pair = generate(name, challenge, requireStrongBox, if (singleUse) 1 else null)
    return try {
      val chain = keyStore().getCertificateChain(name)
      if (chain.isNullOrEmpty()) {
        throw NativePluginException(
          "attestation_unavailable",
          "The keystore returned no attestation certificate chain.",
        )
      }
      mapOf(
        "publicKey" to EcPointEncoder.toUncompressed(pair.public as ECPublicKey),
        "certificateChain" to chain.map { it.encoded },
        "securityLevel" to securityLevel(pair.private),
      )
    } catch (error: Throwable) {
      deleteQuietly(name)
      throw error
    }
  }

  fun sign(alias: String, data: ByteArray): ByteArray {
    if (data.size > maxSignBytes) {
      throw NativePluginException(
        "data_too_large",
        "data must be at most $maxSignBytes bytes.",
      )
    }
    val signature = Signature.getInstance("SHA256withECDSA")
    signature.initSign(privateKey(KeyAliases.namespaced(alias)))
    signature.update(data)
    return signature.sign()
  }

  fun deleteKey(alias: String) {
    keyStore().deleteEntry(KeyAliases.namespaced(alias))
  }

  fun keyInfo(alias: String): Map<String, Any?> {
    val level = securityLevel(privateKey(KeyAliases.namespaced(alias)))
    return mapOf(
      "securityLevel" to level,
      "isHardwareBacked" to (level == "tee" || level == "strongBox"),
    )
  }

  fun deviceCapabilities(): Map<String, Any?> = mapOf(
    "hasStrongBox" to packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE),
    "apiLevel" to Build.VERSION.SDK_INT,
    "supportsMaxUsageCount" to supportsMaxUsageCount(),
    "hasSingleUseKeyFeature" to hasFeature("android.hardware.keystore.single_use_key"),
    "hasLimitedUseKeyFeature" to hasFeature("android.hardware.keystore.limited_use_key"),
  )

  // Creates `count` P-256 keys named "$aliasPrefix$index" and times each generation.
  fun generateSingleUseKeys(
    aliasPrefix: String,
    count: Int,
    requireStrongBox: Boolean,
    challenge: ByteArray?,
    applyMaxUsageCount: Boolean,
  ): Map<String, Any?> {
    if (count !in 1..maxBatchCount) {
      throw NativePluginException(
        "invalid_count",
        "count must be between 1 and $maxBatchCount.",
      )
    }
    val applied = applyMaxUsageCount && supportsMaxUsageCount()
    val names = (0 until count).map { KeyAliases.namespaced("$aliasPrefix$it") }
    names.forEach(::ensureFree)
    val created = ArrayList<String>(count)
    val durationsMs = ArrayList<Long>(count)
    try {
      for (name in names) {
        val start = SystemClock.elapsedRealtimeNanos()
        generate(name, challenge, requireStrongBox, if (applied) 1 else null)
        created.add(name)
        durationsMs.add((SystemClock.elapsedRealtimeNanos() - start) / 1_000_000)
      }
    } catch (error: Throwable) {
      created.forEach(::deleteQuietly)
      throw error
    }
    val aliases = (0 until count).map { "$aliasPrefix$it" }
    return mapOf(
      "aliases" to aliases,
      "durationsMs" to durationsMs,
      "maxUsageCountApplied" to applied,
    )
  }

  private fun generate(
    alias: String,
    challenge: ByteArray?,
    requireStrongBox: Boolean,
    maxUsageCount: Int?,
  ): KeyPair {
    if (requireStrongBox &&
      !packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
    ) {
      throw NativePluginException(
        "strongbox_unavailable",
        "StrongBox was requested but is not available on this device.",
      )
    }
    val builder = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
      .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
      .setDigests(KeyProperties.DIGEST_SHA256)
      .setUserAuthenticationRequired(false)
    if (challenge != null) {
      builder.setAttestationChallenge(challenge)
    }
    if (requireStrongBox) {
      builder.setIsStrongBoxBacked(true)
    }
    if (maxUsageCount != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      builder.setMaxUsageCount(maxUsageCount)
    }
    val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
    generator.initialize(builder.build())
    return StrongBoxGuard.generate(requireStrongBox) { generator.generateKeyPair() }
  }

  private fun ensureFree(name: String) {
    if (keyStore().containsAlias(name)) {
      throw NativePluginException("alias_exists", "A key with this alias already exists.")
    }
  }

  private fun deleteQuietly(name: String) {
    runCatching { keyStore().deleteEntry(name) }
  }

  private fun securityLevel(key: PrivateKey): String {
    val info = KeyFactory.getInstance(key.algorithm, "AndroidKeyStore")
      .getKeySpec(key, KeyInfo::class.java)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      return when (info.securityLevel) {
        KeyProperties.SECURITY_LEVEL_STRONGBOX -> "strongBox"
        KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT -> "tee"
        KeyProperties.SECURITY_LEVEL_SOFTWARE -> "software"
        else -> "unknown"
      }
    }
    // Before API 31 StrongBox cannot be told apart from TEE.
    @Suppress("DEPRECATION")
    return if (info.isInsideSecureHardware) "tee" else "software"
  }

  private fun supportsMaxUsageCount(): Boolean =
    Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

  private fun hasFeature(name: String): Boolean = packageManager.hasSystemFeature(name)

  private fun privateKey(alias: String): PrivateKey =
    keyStore().getKey(alias, null) as? PrivateKey
      ?: throw NativePluginException("key_not_found", "No key for alias $alias.")

  private fun keyStore(): KeyStore =
    KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
}
