import 'package:flutter_modular/flutter_modular.dart';

import '../../core/keys/hardware_key_lab_service.dart';
import '../../core/keys/payment_key_signer.dart';
import '../../core/routes/app_routes.dart';
import '../../core/server/client/server_client.dart';
import '../../core/server/trust/server_trust_service.dart';
import 'presentation/pages/bluetooth_lab_page.dart';
import 'presentation/pages/channel_lab_page.dart';
import 'presentation/pages/clock_lab_page.dart';
import 'presentation/pages/hardware_key_lab_page.dart';
import 'presentation/pages/l2cap_payer_lab_page.dart';
import 'presentation/pages/l2cap_receiver_lab_page.dart';
import 'presentation/pages/lab_page.dart';
import 'presentation/pages/server_lab_page.dart';
import 'presentation/view_models/bluetooth_lab_view_model.dart';
import 'presentation/view_models/channel_lab_view_model.dart';
import 'presentation/view_models/clock_lab_view_model.dart';
import 'presentation/view_models/hardware_key_lab_view_model.dart';
import 'presentation/view_models/l2cap_payer_lab_view_model.dart';
import 'presentation/view_models/l2cap_receiver_lab_view_model.dart';
import 'presentation/view_models/server_lab_view_model.dart';

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
      )
      ..route(
        AppRoutes.labHardwareKeyPath,
        provide: (scope) {
          scope.addChangeNotifier<HardwareKeyLabViewModel>(
            HardwareKeyLabViewModel.new,
          );
        },
        child: (_, _) => const HardwareKeyLabPage(),
      )
      ..route(
        AppRoutes.labServerPath,
        provide: (scope) {
          scope.addChangeNotifier<ServerLabViewModel>(
            (
              ServerClient client,
              HardwareKeyLabService keys,
              PaymentKeySigner signer,
              ServerTrustService trust,
            ) => ServerLabViewModel(client, keys, signer, trust),
          );
        },
        child: (_, _) => const ServerLabPage(),
      );
  },
);
