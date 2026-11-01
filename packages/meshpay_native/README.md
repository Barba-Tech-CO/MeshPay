# meshpay_native

Android-only Flutter plugin for MeshPay BLE advertising, scanning, L2CAP CoC,
monotonic clock samples, and the platform-channel loopback benchmark. Import
public types from their individual files under `lib/`; this package has no
barrel exports.

The app must request Bluetooth runtime permissions. The plugin declares the
platform permissions but never prompts for them. The service-data UUID is
`0e4167c4-bc16-418c-b435-0e3340ea94ad`; advertised session identifiers must be
at most 8 bytes, represented as hexadecimal.

Core API: `MeshpayNative.readClock`, `getBluetoothStatus`,
`startAdvertising`/`stopAdvertising`, `startScan`/`stopScan`,
`listenL2cap`/`stopListenL2cap`, `openL2cap`, `sendL2cap`, `closeL2cap`,
`events`, `setEchoHandler`, and `runLoopbackEcho`.
