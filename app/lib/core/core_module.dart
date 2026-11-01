import 'package:flutter_modular/flutter_modular.dart';
import 'package:meshpay_native/meshpay_native.dart';

import 'ble/meshpay_native_lab_service.dart';
import 'ble/native_lab_service.dart';
import 'ble/permission_handler_service.dart';
import 'ble/permission_service.dart';
import 'clock/clock_log_repository.dart';
import 'clock/json_lines_clock_log_repository.dart';
import 'storage/app_support_directory_service.dart';
import 'storage/path_provider_app_support_directory_service.dart';

final coreModule = createModule(
  register: (context) {
    context
      ..addSingleton<MeshpayNative>(MeshpayNative.new)
      ..addSingleton<NativeLabService>(MeshpayNativeLabService.new)
      ..addSingleton<PermissionService>(PermissionHandlerService.new)
      ..addSingleton<AppSupportDirectoryService>(
        PathProviderAppSupportDirectoryService.new,
      )
      ..addSingleton<ClockLogRepository>(JsonLinesClockLogRepository.new);
  },
);
