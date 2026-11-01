import 'package:flutter_modular/flutter_modular.dart';

import '../../core/routes/app_routes.dart';
import 'presentation/pages/bluetooth_lab_page.dart';
import 'presentation/pages/channel_lab_page.dart';
import 'presentation/pages/clock_lab_page.dart';
import 'presentation/pages/lab_page.dart';
import 'presentation/view_models/bluetooth_lab_view_model.dart';
import 'presentation/view_models/channel_lab_view_model.dart';
import 'presentation/view_models/clock_lab_view_model.dart';

final labModule = createModule(
  path: AppRoutes.lab,
  register: (context) {
    context
      ..route(AppRoutes.root, child: (_, _) => const LabPage())
      ..route(
        AppRoutes.labClockPath,
        provide: (scope) {
          scope.addChangeNotifier<ClockLabViewModel>(ClockLabViewModel.new);
        },
        child: (_, _) => const ClockLabPage(),
      )
      ..route(
        AppRoutes.labChannelPath,
        provide: (scope) {
          scope.addChangeNotifier<ChannelLabViewModel>(
            ChannelLabViewModel.new,
          );
        },
        child: (_, _) => const ChannelLabPage(),
      )
      ..route(
        AppRoutes.labBluetoothPath,
        provide: (scope) {
          scope.addChangeNotifier<BluetoothLabViewModel>(
            BluetoothLabViewModel.new,
          );
        },
        child: (_, _) => const BluetoothLabPage(),
      );
  },
);
