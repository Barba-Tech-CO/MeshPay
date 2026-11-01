import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/pages/bluetooth_lab_page.dart';
import 'package:meshpay/features/lab/presentation/view_models/bluetooth_lab_view_model.dart';
import 'package:meshpay_native/bluetooth_status.dart';
import 'package:meshpay_native/scan_result_event.dart';

import '../../../../fakes/fake_native_lab_service.dart';
import '../../../../fakes/fake_permission_service.dart';

void main() {
  testWidgets('operates Bluetooth controls and displays measured results', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 4200);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final native = FakeNativeLabService();
    final permissions = FakePermissionService();
    final viewModel = BluetoothLabViewModel(native, permissions);
    final module = createModule(register: (_) {});

    await tester.pumpWidget(
      ModularApp(
        module: module,
        provide: (scope) {
          scope.addChangeNotifier<BluetoothLabViewModel>(() => viewModel);
        },
        child: const MaterialApp(home: BluetoothLabPage()),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('LE 2M PHY'), findsOneWidget);
    await tester.tap(find.text('Solicitar permissões'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Iniciar anúncio'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Iniciar busca'));
    await tester.pumpAndSettle();

    native.eventsController.add(
      const ScanResultEvent(
        deviceId: 'AA:BB',
        rssi: -42,
        sessionIdHex: '0123456789abcdef',
        elapsedRealtimeNanos: 2_000_000,
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('AA:BB'), findsOneWidget);
    expect(find.text('Tempo até a primeira descoberta (ms)'), findsOneWidget);
    await tester.tap(find.text('AA:BB'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField).at(1), '');
    await tester.enterText(find.byType(TextField).at(2), '25');
    await tester.tap(find.text('Escutar'));
    await tester.pumpAndSettle();
    expect(find.text('PSM do servidor'), findsOneWidget);

    await tester.tap(find.text('Conectar'));
    await tester.pumpAndSettle();
    expect(find.textContaining('AA:BB'), findsWidgets);
    await tester.tap(find.text('Eco 1 KB x 20'));
    await tester.pumpAndSettle();
    expect(find.text('20 de 20'), findsOneWidget);

    await tester.tap(find.text('Parar busca'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Parar de escutar'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Parar anúncio'));
    await tester.pumpAndSettle();

    permissions.granted = false;
    await tester.tap(find.text('Solicitar permissões'));
    await tester.pumpAndSettle();
    expect(find.textContaining('não foram concedidas'), findsOneWidget);

    native.bluetoothStatus = const BluetoothStatus(
      adapterEnabled: false,
      permissionsGranted: false,
      le2mPhySupported: false,
      leCodedPhySupported: false,
      leExtendedAdvertisingSupported: false,
      multipleAdvertisementSupported: false,
      sdkInt: 30,
    );
    await viewModel.initialize();
    await tester.pumpAndSettle();
    expect(find.text('Não'), findsNWidgets(6));
    expect(find.text('30'), findsOneWidget);
  });
}
