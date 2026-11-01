import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/app_module.dart';
import 'package:meshpay/core/ble/native_lab_service.dart';
import 'package:meshpay/core/ble/permission_service.dart';
import 'package:meshpay/core/clock/clock_log_repository.dart';
import 'package:meshpay/core/routes/app_routes.dart';
import 'package:meshpay/core/storage/app_support_directory_service.dart';
import 'package:meshpay/features/lab/presentation/view_models/bluetooth_lab_view_model.dart';
import 'package:meshpay/features/lab/presentation/view_models/channel_lab_view_model.dart';
import 'package:meshpay/features/lab/presentation/view_models/clock_lab_view_model.dart';
import 'package:meshpay_native/meshpay_native.dart';

void main() {
  test('all lab routes are registered and root redirects to lab', () {
    final bootstrap = bootstrapModule(appModule);
    final labMatch = bootstrap.routes.match(Uri.parse(AppRoutes.lab));
    final clockMatch = bootstrap.routes.match(Uri.parse(AppRoutes.labClock));
    final channelMatch = bootstrap.routes.match(
      Uri.parse(AppRoutes.labChannel),
    );
    final bluetoothMatch = bootstrap.routes.match(
      Uri.parse(AppRoutes.labBluetooth),
    );
    final rootMatch = bootstrap.routes.match(Uri.parse(AppRoutes.root));

    expect(labMatch?.last.route.path, AppRoutes.lab);
    expect(clockMatch?.last.route.path, AppRoutes.labClock);
    expect(channelMatch?.last.route.path, AppRoutes.labChannel);
    expect(bluetoothMatch?.last.route.path, AppRoutes.labBluetooth);
    expect(rootMatch?.last.route.path, AppRoutes.root);
    expect(
      rootMatch?.last.route.guards.single(
        RouteState(uri: Uri.parse(AppRoutes.root)),
      ),
      AppRoutes.lab,
    );
  });

  test('lab routes register page-scoped view models', () {
    final routes = bootstrapModule(appModule).routes;
    final routeViewModels = <(String, Type)>[
      (AppRoutes.labClock, ClockLabViewModel),
      (AppRoutes.labChannel, ChannelLabViewModel),
      (AppRoutes.labBluetooth, BluetoothLabViewModel),
    ];

    for (final (path, viewModelType) in routeViewModels) {
      final levels = routes.match(Uri.parse(path));
      expect(levels, isNotNull);
      if (levels == null) {
        continue;
      }

      final provide = levels.last.route.provide;
      expect(provide, isNotNull);
      if (provide == null) {
        continue;
      }

      final scoped = Scoped();
      provide(scoped);
      expect(scoped.specs.single.type, viewModelType);
    }
  });

  test('core singletons stay outside feature-scoped lifecycles', () {
    final bootstrap = bootstrapModule(appModule);
    final nativePlugin = bootstrap.injector.get<MeshpayNative>();
    final nativeService = bootstrap.injector.get<NativeLabService>();
    final permissionService = bootstrap.injector.get<PermissionService>();
    final clockLogRepository = bootstrap.injector.get<ClockLogRepository>();
    final directoryService = bootstrap.injector
        .get<AppSupportDirectoryService>();

    expect(bootstrap.injector.get<MeshpayNative>(), same(nativePlugin));
    expect(bootstrap.injector.get<NativeLabService>(), same(nativeService));
    expect(
      bootstrap.injector.get<PermissionService>(),
      same(permissionService),
    );
    expect(
      bootstrap.injector.get<ClockLogRepository>(),
      same(clockLogRepository),
    );
    expect(
      bootstrap.injector.get<AppSupportDirectoryService>(),
      same(directoryService),
    );
    expect(nativePlugin, isNot(isA<Disposable>()));
    expect(nativeService, isNot(isA<Disposable>()));
    expect(permissionService, isNot(isA<Disposable>()));
    expect(clockLogRepository, isNot(isA<Disposable>()));
    expect(directoryService, isNot(isA<Disposable>()));
  });
}
