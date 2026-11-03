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

## Security

The L2CAP channel is neither authenticated nor encrypted.
This plugin is currently used only for laboratory measurements (echo of random bytes).
NO payment data may travel over it without an application-layer authenticated and encrypted protocol bound to the session identifier.

## Hardware payment keys (Android Keystore)

`generateAttestedPaymentKey` (EC P-256, attestation challenge, optional
StrongBox, optional `setMaxUsageCount(1)`), `signWithPaymentKey`
(SHA256withECDSA, DER), `deletePaymentKey`, `getPaymentKeyInfo`
(`software`/`tee`/`strongBox`), `getDeviceCapabilities` and
`generateSingleUseKeys` (per-key generation time in ms, PV-CRIP-04).
`strongbox_unavailable` is raised when StrongBox is required but missing;
there is no silent fallback. User authentication is off (lab only).
`setMaxUsageCount` needs API 31; below that `singleUse` fails with
`max_usage_count_unsupported` and `generateSingleUseKeys` reports
`maxUsageCountApplied: false`. Below API 31 StrongBox is reported as `tee`.
Keystore calls run on a worker thread created on attach and shut down on detach.
Aliases are logical on the Dart side (1..128 chars of `A-Za-z0-9._-`) and stored
as `meshpay.<alias>`. Generation never overwrites (`alias_exists`; batches check
all aliases first), batches are capped at 200 keys (`invalid_count`), signed data
at 64 KiB (`data_too_large`), a missing attestation chain fails with
`attestation_unavailable`, and a key whose creation flow fails afterwards is
deleted. Errors surface in Dart as `PaymentKeyException` with the native code.

Keys have no user authentication by design (laboratory only; lab phones need no
lock screen). Consequently `setInvalidatedByBiometricEnrollment` is not
applicable. Production keys must revisit this.

Verified: Dart channel contract and error codes (`test/payment_key_test.dart`);
Kotlin unit tests for point encoding, error mapping and alias validation were
written but not run (no gradle).
Only verifiable on a device: real attestation chains, StrongBox behavior,
security levels, generation latency, usage-count enforcement (the Android 10
test phone has no StrongBox and no `setMaxUsageCount`).
