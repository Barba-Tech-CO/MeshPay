import 'package:flutter/widgets.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:meshpay_native/meshpay_native.dart';

import 'app_module.dart';
import 'app_widget.dart';
import 'core/clock/json_lines_clock_log_repository.dart';
import 'core/storage/path_provider_app_support_directory_service.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final native = MeshpayNative();
  final clockLogRepository = JsonLinesClockLogRepository(
    PathProviderAppSupportDirectoryService(),
  );
  await clockLogRepository.append(
    sample: await native.readClock(),
    label: 'Início do app',
    isAppLaunch: true,
  );
  runApp(
    ModularApp(
      module: appModule,
      child: const AppWidget(),
    ),
  );
}
