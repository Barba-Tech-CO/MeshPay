import 'package:flutter_modular/flutter_modular.dart';
import 'package:meshpay_native/meshpay_native.dart';

import 'ble/meshpay_native_lab_service.dart';
import 'ble/native_lab_service.dart';
import 'ble/permission_handler_service.dart';
import 'ble/permission_service.dart';
import 'clock/clock_log_repository.dart';
import 'clock/json_lines_clock_log_repository.dart';
import 'noise/meshpay_core_noise_service.dart';
import 'noise/noise_service.dart';
import 'storage/app_support_directory_service.dart';
import 'storage/path_provider_app_support_directory_service.dart';
import 'tx/meshpay_core_payer_tx_engine_service.dart';
import 'tx/meshpay_core_receiver_tx_engine_service.dart';
import 'tx/payer_tx_engine_service.dart';
import 'tx/receiver_tx_engine_service.dart';

final coreModule = createModule(
  register: (context) {
    context
      ..addSingleton<MeshpayNative>(MeshpayNative.new)
      ..addSingleton<NativeLabService>(MeshpayNativeLabService.new)
      ..addSingleton<PermissionService>(PermissionHandlerService.new)
      ..addSingleton<NoiseService>(MeshpayCoreNoiseService.new)
      ..addSingleton<AppSupportDirectoryService>(
        PathProviderAppSupportDirectoryService.new,
      )
      ..addSingleton<PayerTxEngineService>(
        MeshpayCorePayerTxEngineService.new,
      )
      ..addSingleton<ReceiverTxEngineService>(
        MeshpayCoreReceiverTxEngineService.new,
      )
      ..addSingleton<ClockLogRepository>(JsonLinesClockLogRepository.new);
  },
);
