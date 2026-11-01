import 'package:meshpay/core/ble/permission_service.dart';

class FakePermissionService implements PermissionService {
  bool granted = true;
  Object? requestError;
  Object? checkError;

  @override
  Future<bool> hasBluetoothPermissions() async {
    if (checkError case final error?) {
      throw error;
    }
    return granted;
  }

  @override
  Future<bool> requestBluetoothPermissions() async {
    if (requestError case final error?) {
      throw error;
    }
    return granted;
  }
}
