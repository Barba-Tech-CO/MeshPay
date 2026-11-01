package company.barbatech.meshpay_native

import android.bluetooth.BluetoothSocket
import java.util.concurrent.Executors

class L2capConnection(
  val channelId: Int,
  val socket: BluetoothSocket,
  val fromServer: Boolean,
) {
  val writer = Executors.newSingleThreadExecutor()
}
