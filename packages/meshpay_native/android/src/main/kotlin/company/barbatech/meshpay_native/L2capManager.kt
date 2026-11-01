package company.barbatech.meshpay_native

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothServerSocket
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.os.Handler
import android.os.Looper
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.atomic.AtomicInteger

class L2capManager(
  context: Context,
  private val events: NativeEventDispatcher,
) {
  private val adapter =
    context.getSystemService(BluetoothManager::class.java)?.adapter
  private val permissions = BluetoothPermissionChecker(context)
  private val mainHandler = Handler(Looper.getMainLooper())
  private val channelIds = AtomicInteger(1)
  private val connections = ConcurrentHashMap<Int, L2capConnection>()
  private val serverConnectionLimiter =
    L2capConnectionLimiter(L2capConnectionLimiter.maxServerConnections)
  private val serverLock = Any()

  @Volatile
  private var serverSocket: BluetoothServerSocket? = null

  fun listen(): Int {
    permissions.requireConnectPermission()
    val currentAdapter = requireEnabledAdapter()
    val server = synchronized(serverLock) {
      if (serverSocket != null) {
        throw NativePluginException(
          "l2cap_already_listening",
          "An L2CAP server is already listening.",
        )
      }
      currentAdapter.listenUsingInsecureL2capChannel().also {
        serverSocket = it
      }
    }
    Thread(
      { acceptConnections(server) },
      "meshpay-l2cap-accept",
    ).start()
    return server.psm
  }

  fun stopListening() {
    permissions.requireConnectPermission()
    val server = synchronized(serverLock) {
      serverSocket.also { serverSocket = null }
    } ?: return
    server.close()
  }

  fun openClient(
    deviceId: String,
    psm: Int,
    onComplete: (Int?, NativePluginException?) -> Unit,
  ) {
    permissions.requireConnectPermission()
    if (psm !in 1..65535) {
      throw NativePluginException("invalid_psm", "The L2CAP PSM is out of range.")
    }
    val currentAdapter = requireEnabledAdapter()
    currentAdapter.cancelDiscovery()
    Thread(
      { connectClient(currentAdapter, deviceId, psm, onComplete) },
      "meshpay-l2cap-connect",
    ).start()
  }

  fun send(
    channelId: Int,
    data: ByteArray,
    onComplete: (NativePluginException?) -> Unit,
  ) {
    val connection = connections[channelId]
      ?: throw NativePluginException(
        "invalid_channel",
        "The L2CAP channel is not open.",
      )
    try {
      connection.writer.execute {
        try {
          connection.socket.outputStream.apply {
            write(data)
            flush()
          }
          complete(onComplete, null)
        } catch (error: IOException) {
          finishConnection(channelId, true)
          complete(
            onComplete,
            NativePluginException("l2cap_io_error", "Writing to L2CAP failed."),
          )
        } catch (error: SecurityException) {
          finishConnection(channelId, true)
          complete(
            onComplete,
            NativePluginException("permission_denied", "Bluetooth permission was revoked."),
          )
        }
      }
    } catch (error: RejectedExecutionException) {
      complete(
        onComplete,
        NativePluginException("channel_closed", "The L2CAP channel is closing."),
      )
    }
  }

  fun close(channelId: Int) {
    val connection = connections.remove(channelId)
      ?: throw NativePluginException(
        "invalid_channel",
        "The L2CAP channel is not open.",
      )
    if (connection.fromServer) {
      serverConnectionLimiter.release()
    }
    connection.writer.shutdownNow()
    var closeFailure: IOException? = null
    try {
      connection.socket.close()
    } catch (error: IOException) {
      closeFailure = error
    }
    if (closeFailure != null) {
      events.emitError(
        "l2cap_io_error",
        "Closing the L2CAP channel failed.",
        channelId,
      )
    }
    emitClosed(channelId)
    if (closeFailure != null) {
      throw NativePluginException(
        "l2cap_io_error",
        "Closing the L2CAP channel failed.",
      )
    }
  }

  fun dispose() {
    val server = synchronized(serverLock) {
      serverSocket.also { serverSocket = null }
    }
    try {
      server?.close()
    } catch (error: IOException) {
      events.emitError("l2cap_io_error", "Closing the L2CAP server failed.")
    }
    connections.keys.toList().forEach { channelId ->
      finishConnection(channelId, false)
    }
  }

  private fun acceptConnections(server: BluetoothServerSocket) {
    while (serverSocket === server) {
      try {
        val socket = server.accept()
        if (!serverConnectionLimiter.tryAcquire()) {
          try {
            socket.close()
          } catch (error: IOException) {
            events.emitError("l2cap_io_error", "Closing a rejected L2CAP socket failed.")
          }
          events.emitError(
            "l2cap_too_many_connections",
            "Too many L2CAP server connections are open.",
          )
          continue
        }
        registerConnection(socket, "server")
      } catch (error: IOException) {
        if (serverSocket === server) {
          events.emitError("l2cap_io_error", "Accepting an L2CAP channel failed.")
        }
        return
      } catch (error: SecurityException) {
        events.emitError("permission_denied", "Bluetooth permission was revoked.")
        return
      }
    }
  }

  private fun connectClient(
    currentAdapter: BluetoothAdapter,
    deviceId: String,
    psm: Int,
    onComplete: (Int?, NativePluginException?) -> Unit,
  ) {
    var socket: BluetoothSocket? = null
    try {
      val device = currentAdapter.getRemoteDevice(deviceId)
      val clientSocket = device.createInsecureL2capChannel(psm)
      socket = clientSocket
      clientSocket.connect()
      val channelId = registerConnection(clientSocket, "client")
      complete(onComplete, channelId, null)
    } catch (error: IllegalArgumentException) {
      failUnconnectedSocket(socket)
      complete(
        onComplete,
        null,
        NativePluginException("invalid_device_id", "The Bluetooth device ID is invalid."),
      )
    } catch (error: IOException) {
      failUnconnectedSocket(socket)
      complete(
        onComplete,
        null,
        NativePluginException("l2cap_connect_failed", "Opening the L2CAP channel failed."),
      )
    } catch (error: SecurityException) {
      failUnconnectedSocket(socket)
      complete(
        onComplete,
        null,
        NativePluginException("permission_denied", "Bluetooth permission was revoked."),
      )
    }
  }

  private fun registerConnection(socket: BluetoothSocket, role: String): Int {
    val channelId = channelIds.getAndIncrement()
    val fromServer = role == "server"
    var registered = false
    try {
      val newConnection = L2capConnection(channelId, socket, fromServer)
      val deviceId = socket.remoteDevice.address
      connections[channelId] = newConnection
      registered = true
      events.emit(
        mapOf(
          "type" to "l2capOpened",
          "channelId" to channelId,
          "role" to role,
          "deviceId" to deviceId,
        ),
      )
      Thread(
        { readConnection(newConnection) },
        "meshpay-l2cap-reader-$channelId",
      ).start()
      return channelId
    } catch (error: RuntimeException) {
      if (registered) {
        finishConnection(channelId, false)
      } else {
        if (fromServer) {
          serverConnectionLimiter.release()
        }
        failUnconnectedSocket(socket)
      }
      throw error
    }
  }

  private fun readConnection(connection: L2capConnection) {
    val buffer = ByteArray(8192)
    try {
      val input = connection.socket.inputStream
      while (true) {
        val count = input.read(buffer)
        if (count < 0) {
          finishConnection(connection.channelId, false)
          return
        }
        if (count > 0) {
          events.emit(
            mapOf(
              "type" to "l2capData",
              "channelId" to connection.channelId,
              "data" to buffer.copyOf(count),
            ),
          )
        }
      }
    } catch (error: IOException) {
      finishConnection(connection.channelId, true)
    } catch (error: SecurityException) {
      events.emitError(
        "permission_denied",
        "Bluetooth permission was revoked.",
        connection.channelId,
      )
      finishConnection(connection.channelId, false)
    }
  }

  private fun finishConnection(channelId: Int, reportIoFailure: Boolean) {
    val connection = connections.remove(channelId) ?: return
    if (connection.fromServer) {
      serverConnectionLimiter.release()
    }
    connection.writer.shutdownNow()
    try {
      connection.socket.close()
    } catch (error: IOException) {
      events.emitError(
        "l2cap_io_error",
        "Closing the L2CAP channel failed.",
        channelId,
      )
    }
    if (reportIoFailure) {
      events.emitError(
        "l2cap_io_error",
        "The L2CAP channel closed after an I/O failure.",
        channelId,
      )
    }
    emitClosed(channelId)
  }

  private fun failUnconnectedSocket(socket: BluetoothSocket?) {
    try {
      socket?.close()
    } catch (error: IOException) {
      events.emitError("l2cap_io_error", "Closing a failed L2CAP socket failed.")
    }
  }

  private fun complete(
    callback: (Int?, NativePluginException?) -> Unit,
    channelId: Int?,
    error: NativePluginException?,
  ) {
    mainHandler.post { callback(channelId, error) }
  }

  private fun complete(
    callback: (NativePluginException?) -> Unit,
    error: NativePluginException?,
  ) {
    mainHandler.post { callback(error) }
  }

  private fun emitClosed(channelId: Int) {
    events.emit(mapOf("type" to "l2capClosed", "channelId" to channelId))
  }

  private fun requireEnabledAdapter(): BluetoothAdapter {
    val currentAdapter = adapter
      ?: throw NativePluginException(
        "bluetooth_unavailable",
        "This device does not provide a Bluetooth adapter.",
      )
    if (!currentAdapter.isEnabled) {
      throw NativePluginException("bluetooth_disabled", "Bluetooth is disabled.")
    }
    return currentAdapter
  }
}
