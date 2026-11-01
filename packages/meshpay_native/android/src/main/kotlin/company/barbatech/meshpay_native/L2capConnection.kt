package company.barbatech.meshpay_native

import android.bluetooth.BluetoothSocket
import java.util.concurrent.Executors

class L2capConnection(
  val channelId: Int,
  val socket: BluetoothSocket,
) {
  val writer = Executors.newSingleThreadExecutor()
}
