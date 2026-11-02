import 'package:flutter_modular/flutter_modular.dart';

import '../../core/routes/app_routes.dart';
import 'presentation/pages/bluetooth_lab_page.dart';
import 'presentation/pages/channel_lab_page.dart';
import 'presentation/pages/clock_lab_page.dart';
import 'presentation/pages/l2cap_payer_lab_page.dart';
import 'presentation/pages/l2cap_receiver_lab_page.dart';
import 'presentation/pages/lab_page.dart';
import 'presentation/view_models/bluetooth_lab_view_model.dart';
import 'presentation/view_models/channel_lab_view_model.dart';
import 'presentation/view_models/clock_lab_view_model.dart';
import 'presentation/view_models/l2cap_payer_lab_view_model.dart';
import 'presentation/view_models/l2cap_receiver_lab_view_model.dart';

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
      )
      ..route(
        AppRoutes.labL2capReceiverPath,
        provide: (scope) {
          scope.addChangeNotifier<L2capReceiverLabViewModel>(
            L2capReceiverLabViewModel.new,
          );
        },
        child: (_, _) => const L2capReceiverLabPage(),
      )
      ..route(
        AppRoutes.labL2capPayerPath,
        provide: (scope) {
          scope.addChangeNotifier<L2capPayerLabViewModel>(
            L2capPayerLabViewModel.new,
          );
        },
        child: (_, _) => const L2capPayerLabPage(),
      );
  },
);
