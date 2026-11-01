package company.barbatech.meshpay_native

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.bluetooth.le.BluetoothLeScanner
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid

class BluetoothLeController(
  context: Context,
  private val events: NativeEventDispatcher,
) {
  private val adapter =
    context.getSystemService(BluetoothManager::class.java)?.adapter
  private val permissions = BluetoothPermissionChecker(context)
  private val mainHandler = Handler(Looper.getMainLooper())
  private val serviceUuid = ParcelUuid.fromString(
    "0e4167c4-bc16-418c-b435-0e3340ea94ad",
  )

  private var advertisingClient: BluetoothLeAdvertiser? = null
  private var advertisingCallback: AdvertiseCallback? = null
  private var scanClient: BluetoothLeScanner? = null
  private var scanCallback: ScanCallback? = null
  private var expectedSessionId: ByteArray? = null

  @Synchronized
  fun getStatus(): Map<String, Any> {
    val canQueryAdapter = permissions.hasConnectPermission()
    val currentAdapter = adapter
    val canReadFeatures = currentAdapter != null && canQueryAdapter
    return mapOf(
      "adapterEnabled" to (canReadFeatures && currentAdapter?.isEnabled == true),
      "permissionsGranted" to permissions.hasAllPermissions(),
      "le2mPhySupported" to
        (canReadFeatures && currentAdapter?.isLe2MPhySupported == true),
      "leCodedPhySupported" to
        (canReadFeatures && currentAdapter?.isLeCodedPhySupported == true),
      "leExtendedAdvertisingSupported" to
        (canReadFeatures && currentAdapter?.isLeExtendedAdvertisingSupported == true),
      "multipleAdvertisementSupported" to
        (canReadFeatures && currentAdapter?.isMultipleAdvertisementSupported == true),
      "sdkInt" to android.os.Build.VERSION.SDK_INT,
    )
  }

  @Synchronized
  fun startAdvertising(
    sessionIdHex: String,
    onComplete: (NativePluginException?) -> Unit,
  ) {
    permissions.requireAdvertisingPermission()
    permissions.requireConnectPermission()
    val currentAdapter = requireEnabledAdapter()
    val sessionId = SessionIdParser.parse(sessionIdHex)
    if (advertisingCallback != null) {
      throw NativePluginException(
        "advertising_already_started",
        "BLE advertising is already active.",
      )
    }

    val advertiser = currentAdapter.bluetoothLeAdvertiser
      ?: throw NativePluginException(
        "bluetooth_unavailable",
        "This adapter does not provide BLE advertising.",
      )
    val callback = object : AdvertiseCallback() {
      override fun onStartSuccess(settingsInEffect: AdvertiseSettings?) {
        mainHandler.post { onComplete(null) }
      }

      override fun onStartFailure(errorCode: Int) {
        synchronized(this@BluetoothLeController) {
          if (advertisingCallback === this) {
            advertisingCallback = null
            advertisingClient = null
          }
        }
        val error = NativePluginException(
          "advertising_failed",
          "BLE advertising failed with code $errorCode.",
        )
        events.emitError(error.code, error.message ?: "BLE advertising failed.")
        mainHandler.post { onComplete(error) }
      }
    }
    val settings = AdvertiseSettings.Builder()
      .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
      .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
      .build()
    val data = AdvertiseData.Builder()
      .addServiceData(serviceUuid, sessionId)
      .setIncludeDeviceName(false)
      .setIncludeTxPowerLevel(false)
      .build()

    advertisingClient = advertiser
    advertisingCallback = callback
    try {
      advertiser.startAdvertising(settings, data, callback)
    } catch (error: IllegalArgumentException) {
      advertisingCallback = null
      advertisingClient = null
      throw NativePluginException("advertising_failed", "BLE advertising could not start.")
    } catch (error: IllegalStateException) {
      advertisingCallback = null
      advertisingClient = null
      throw NativePluginException("advertising_failed", "BLE advertising could not start.")
    } catch (error: SecurityException) {
      advertisingCallback = null
      advertisingClient = null
      throw error
    }
  }

  @Synchronized
  fun stopAdvertising() {
    permissions.requireAdvertisingPermission()
    val callback = advertisingCallback ?: return
    val client = advertisingClient
    advertisingCallback = null
    advertisingClient = null
    client?.stopAdvertising(callback)
  }

  @Synchronized
  fun startScan(sessionIdHex: String?) {
    permissions.requireScanPermission()
    permissions.requireConnectPermission()
    val currentAdapter = requireEnabledAdapter()
    if (scanCallback != null) {
      throw NativePluginException(
        "scan_already_started",
        "A BLE scan is already active.",
      )
    }
    expectedSessionId = sessionIdHex?.let { SessionIdParser.parse(it) }
    val scanner = currentAdapter.bluetoothLeScanner
      ?: throw NativePluginException(
        "bluetooth_unavailable",
        "This adapter does not provide BLE scanning.",
      )
    val filter = ScanFilter.Builder()
      .setServiceData(serviceUuid, expectedSessionId)
      .build()
    val settings = ScanSettings.Builder()
      .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
      .build()
    val callback = createScanCallback()
    scanClient = scanner
    scanCallback = callback
    try {
      scanner.startScan(listOf(filter), settings, callback)
    } catch (error: IllegalArgumentException) {
      clearScanState()
      throw NativePluginException("scan_failed", "BLE scanning could not start.")
    } catch (error: IllegalStateException) {
      clearScanState()
      throw NativePluginException("scan_failed", "BLE scanning could not start.")
    } catch (error: SecurityException) {
      clearScanState()
      throw error
    }
  }

  @Synchronized
  fun stopScan() {
    permissions.requireScanPermission()
    val callback = scanCallback ?: return
    val client = scanClient
    scanCallback = null
    scanClient = null
    expectedSessionId = null
    client?.stopScan(callback)
  }

  @Synchronized
  fun dispose() {
    try {
      advertisingCallback?.let { advertisingClient?.stopAdvertising(it) }
    } catch (error: SecurityException) {
      events.emitError("permission_denied", "Bluetooth permission was revoked.")
    }
    advertisingCallback = null
    advertisingClient = null
    try {
      scanCallback?.let { scanClient?.stopScan(it) }
    } catch (error: SecurityException) {
      events.emitError("permission_denied", "Bluetooth permission was revoked.")
    }
    scanCallback = null
    scanClient = null
    expectedSessionId = null
  }

  private fun requireEnabledAdapter(): BluetoothAdapter {
    val currentAdapter = adapter
      ?: throw NativePluginException(
        "bluetooth_unavailable",
        "This device does not provide a Bluetooth adapter.",
      )
    if (!currentAdapter.isEnabled) {
      throw NativePluginException(
        "bluetooth_disabled",
        "Bluetooth is disabled.",
      )
    }
    return currentAdapter
  }

  private fun createScanCallback(): ScanCallback =
    object : ScanCallback() {
      override fun onScanResult(callbackType: Int, result: ScanResult) {
        emitScanResult(result)
      }

      override fun onScanFailed(errorCode: Int) {
        synchronized(this@BluetoothLeController) {
          clearScanState()
        }
        events.emitError(
          "scan_failed",
          "BLE scanning failed with code $errorCode.",
        )
      }
    }

  private fun emitScanResult(result: ScanResult) {
    val serviceData = result.scanRecord?.getServiceData(serviceUuid) ?: return
    val sessionId = expectedSessionId
    if (sessionId != null && !sessionId.contentEquals(serviceData)) {
      return
    }
    if (serviceData.isEmpty() || serviceData.size > 8) {
      events.emitError(
        "invalid_scan_payload",
        "The BLE service-data session identifier has an invalid length.",
      )
      return
    }
    events.emit(
      mapOf(
        "type" to "scanResult",
        "deviceId" to result.device.address,
        "rssi" to result.rssi,
        "sessionIdHex" to serviceData.toHexString(),
        "elapsedRealtimeNanos" to result.timestampNanos,
      ),
    )
  }

  private fun clearScanState() {
    scanCallback = null
    scanClient = null
    expectedSessionId = null
  }

  private fun ByteArray.toHexString(): String = buildString(size * 2) {
    val digits = "0123456789abcdef"
    for (byte in this@toHexString) {
      val value = byte.toInt() and 0xff
      append(digits[value ushr 4])
      append(digits[value and 0x0f])
    }
  }
}
