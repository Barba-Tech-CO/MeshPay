import 'package:permission_handler/permission_handler.dart';

import 'native_lab_service.dart';
import 'permission_service.dart';

class PermissionHandlerService implements PermissionService {
  PermissionHandlerService(this._nativeService);

  final NativeLabService _nativeService;

  @override
  Future<bool> requestBluetoothPermissions() async {
    final permissions = await _permissionsForCurrentSdk();
    final statuses = await Future.wait(
      permissions.map((permission) => permission.request()),
    );
    return statuses.every((status) => status.isGranted);
  }

  @override
  Future<bool> hasBluetoothPermissions() async {
    final permissions = await _permissionsForCurrentSdk();
    final statuses = await Future.wait(
      permissions.map((permission) => permission.status),
    );
    return statuses.every((status) => status.isGranted);
  }

  Future<List<Permission>> _permissionsForCurrentSdk() async {
    final status = await _nativeService.getBluetoothStatus();
    if (status.sdkInt <= 30) {
      return <Permission>[Permission.location];
    }
    return <Permission>[
      Permission.bluetoothScan,
      Permission.bluetoothAdvertise,
      Permission.bluetoothConnect,
    ];
  }
}
