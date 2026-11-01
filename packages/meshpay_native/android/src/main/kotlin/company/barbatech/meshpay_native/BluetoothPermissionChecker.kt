package company.barbatech.meshpay_native

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build

class BluetoothPermissionChecker(private val context: Context) {
  fun hasAllPermissions(): Boolean =
    hasScanPermission() && hasAdvertisingPermission() && hasConnectPermission()

  fun hasConnectPermission(): Boolean =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      has(Manifest.permission.BLUETOOTH_CONNECT)
    } else {
      hasLegacyBluetoothPermissions()
    }

  fun requireScanPermission() {
    requirePermission(
      hasScanPermission(),
      "Scanning requires Bluetooth and scan permissions.",
    )
  }

  fun requireAdvertisingPermission() {
    requirePermission(
      hasAdvertisingPermission(),
      "Advertising requires Bluetooth advertising permission.",
    )
  }

  fun requireConnectPermission() {
    requirePermission(
      hasConnectPermission(),
      "L2CAP requires Bluetooth connect permission.",
    )
  }

  private fun hasScanPermission(): Boolean =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      has(Manifest.permission.BLUETOOTH_SCAN)
    } else {
      hasLegacyBluetoothPermissions() &&
        has(Manifest.permission.ACCESS_FINE_LOCATION)
    }

  private fun hasAdvertisingPermission(): Boolean =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      has(Manifest.permission.BLUETOOTH_ADVERTISE)
    } else {
      hasLegacyBluetoothPermissions()
    }

  private fun hasLegacyBluetoothPermissions(): Boolean =
    has(Manifest.permission.BLUETOOTH) &&
      has(Manifest.permission.BLUETOOTH_ADMIN)

  private fun has(permission: String): Boolean =
    context.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

  private fun requirePermission(granted: Boolean, message: String) {
    if (!granted) {
      throw NativePluginException("permission_denied", message)
    }
  }
}
