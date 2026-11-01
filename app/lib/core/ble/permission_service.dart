abstract interface class PermissionService {
  Future<bool> requestBluetoothPermissions();

  Future<bool> hasBluetoothPermissions();
}
