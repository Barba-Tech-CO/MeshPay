import 'dart:io';

import 'package:flutter_modular/flutter_modular.dart';
import 'package:meshpay_native/meshpay_native.dart';

import 'ble/meshpay_native_lab_service.dart';
import 'ble/native_lab_service.dart';
import 'ble/permission_handler_service.dart';
import 'ble/permission_service.dart';
import 'clock/clock_log_repository.dart';
import 'clock/json_lines_clock_log_repository.dart';
import 'dtn/dtn_buffer_service.dart';
import 'dtn/meshpay_core_dtn_buffer_service.dart';
import 'keys/hardware_key_lab_service.dart';
import 'keys/meshpay_native_hardware_key_lab_service.dart';
import 'keys/meshpay_native_payment_key_signer.dart';
import 'keys/payment_key_signer.dart';
import 'noise/meshpay_core_noise_service.dart';
import 'noise/noise_service.dart';
import 'storage/app_support_directory_service.dart';
import 'storage/path_provider_app_support_directory_service.dart';
import 'tx/meshpay_core_external_payer_tx_engine_service.dart';
import 'tx/meshpay_core_external_receiver_tx_engine_service.dart';
import 'tx/meshpay_core_payer_tx_engine_service.dart';
import 'tx/meshpay_core_receiver_tx_engine_service.dart';
import 'tx/payer_tx_engine_service.dart';
import 'tx/receiver_tx_engine_service.dart';
import 'tx/tx_signer_config.dart';

final bool _useKeystore =
    TxSignerConfig.useKeystoreOnAndroid && Platform.isAndroid;

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
      ..addSingleton<PaymentKeySigner>(MeshpayNativePaymentKeySigner.new)
      ..addSingleton<HardwareKeyLabService>(
        MeshpayNativeHardwareKeyLabService.new,
      )
      ..addSingleton<PayerTxEngineService>(
        _useKeystore
            ? MeshpayCoreExternalPayerTxEngineService.new
            : MeshpayCorePayerTxEngineService.new,
      )
      ..addSingleton<ReceiverTxEngineService>(
        _useKeystore
            ? MeshpayCoreExternalReceiverTxEngineService.new
            : MeshpayCoreReceiverTxEngineService.new,
      )
      ..addSingleton<DtnBufferService>(MeshpayCoreDtnBufferService.new)
      ..addSingleton<ClockLogRepository>(JsonLinesClockLogRepository.new);
  },
);
