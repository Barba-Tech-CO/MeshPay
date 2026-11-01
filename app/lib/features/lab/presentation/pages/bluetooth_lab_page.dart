import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import '../view_models/bluetooth_lab_view_model.dart';

class BluetoothLabPage extends StatefulWidget {
  const BluetoothLabPage({super.key});

  @override
  State<BluetoothLabPage> createState() => _BluetoothLabPageState();
}

class _BluetoothLabPageState extends State<BluetoothLabPage> {
  final TextEditingController _scanFilterController = TextEditingController();
  final TextEditingController _deviceIdController = TextEditingController();
  final TextEditingController _psmController = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        context.read<BluetoothLabViewModel>().initialize();
      }
    });
  }

  @override
  void dispose() {
    _scanFilterController.dispose();
    _deviceIdController.dispose();
    _psmController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<BluetoothLabViewModel>();
    final bluetoothStatus = viewModel.status;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Bluetooth'),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionCardDs(
            title: 'Estado do Bluetooth',
            child: Column(
              children: [
                MetricRowDs(
                  label: 'Adaptador ativado',
                  value: bluetoothStatus?.adapterEnabled == true
                      ? 'Sim'
                      : 'Não',
                ),
                MetricRowDs(
                  label: 'Permissões concedidas',
                  value: bluetoothStatus == null
                      ? 'Indisponível'
                      : bluetoothStatus.permissionsGranted
                      ? 'Sim'
                      : 'Não',
                ),
                MetricRowDs(
                  label: 'LE 2M PHY',
                  value: bluetoothStatus?.le2mPhySupported == true
                      ? 'Sim'
                      : 'Não',
                ),
                MetricRowDs(
                  label: 'LE Coded PHY',
                  value: bluetoothStatus?.leCodedPhySupported == true
                      ? 'Sim'
                      : 'Não',
                ),
                MetricRowDs(
                  label: 'Anúncio estendido',
                  value: bluetoothStatus?.leExtendedAdvertisingSupported == true
                      ? 'Sim'
                      : 'Não',
                ),
                MetricRowDs(
                  label: 'Múltiplos anúncios',
                  value: bluetoothStatus?.multipleAdvertisementSupported == true
                      ? 'Sim'
                      : 'Não',
                ),
                MetricRowDs(
                  label: 'Versão do Android (SDK)',
                  value: bluetoothStatus == null
                      ? 'Indisponível'
                      : '${bluetoothStatus.sdkInt}',
                  mono: true,
                ),
                const SizedBox(height: 8),
                OutlinedButton(
                  onPressed: viewModel.requestBluetoothPermissions,
                  child: const Text('Solicitar permissões'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Anúncio BLE',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                MetricRowDs(
                  label: 'ID da sessão',
                  value: viewModel.sessionIdHex ?? 'Ainda não gerado',
                  mono: true,
                ),
                const SizedBox(height: 8),
                FilledButton(
                  onPressed: viewModel.isAdvertising
                      ? null
                      : viewModel.startAdvertising,
                  child: const Text('Iniciar anúncio'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isAdvertising
                      ? viewModel.stopAdvertising
                      : null,
                  child: const Text('Parar anúncio'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Busca BLE',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextField(
                  controller: _scanFilterController,
                  onChanged: viewModel.updateScanFilter,
                  decoration: const InputDecoration(
                    labelText: 'Filtrar pelo ID da sessão (opcional)',
                  ),
                ),
                const SizedBox(height: 8),
                FilledButton(
                  onPressed: viewModel.isScanning ? null : viewModel.startScan,
                  child: const Text('Iniciar busca'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isScanning ? viewModel.stopScan : null,
                  child: const Text('Parar busca'),
                ),
                const SizedBox(height: 8),
                if (viewModel.scanResults.isEmpty)
                  const Text('Nenhum dispositivo encontrado.')
                else
                  for (final result in viewModel.scanResults)
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      title: Text(
                        result.deviceId,
                        style: AppFonts.mono(
                          textStyle: Theme.of(context).textTheme.titleMedium,
                        ),
                      ),
                      subtitle: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          MetricRowDs(
                            label: 'RSSI (dBm)',
                            value: '${result.rssi}',
                            mono: true,
                          ),
                          MetricRowDs(
                            label: 'ID da sessão',
                            value: result.sessionIdHex,
                            mono: true,
                          ),
                          MetricRowDs(
                            label: 'Tempo até a primeira descoberta (ms)',
                            value: '${result.firstSightingMilliseconds}',
                            mono: true,
                          ),
                        ],
                      ),
                      onTap: () => viewModel.selectScanResult(result),
                    ),
                if (viewModel.selectedDeviceId case final selectedDeviceId?)
                  MetricRowDs(
                    label: 'Dispositivo selecionado',
                    value: selectedDeviceId,
                    mono: true,
                  ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Canal L2CAP',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                FilledButton(
                  onPressed: viewModel.isListening
                      ? null
                      : viewModel.startL2capListening,
                  child: const Text('Escutar'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isListening
                      ? viewModel.stopL2capListening
                      : null,
                  child: const Text('Parar de escutar'),
                ),
                if (viewModel.l2capPsm case final psm?)
                  MetricRowDs(
                    label: 'PSM do servidor',
                    value: '$psm',
                    mono: true,
                  ),
                TextField(
                  controller: _deviceIdController,
                  decoration: const InputDecoration(
                    labelText: 'ID do dispositivo remoto',
                  ),
                ),
                TextField(
                  controller: _psmController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(labelText: 'PSM remoto'),
                ),
                const SizedBox(height: 8),
                FilledButton(
                  onPressed: () => viewModel.connectL2cap(
                    deviceId: _deviceIdController.text,
                    psm: _psmController.text,
                  ),
                  child: const Text('Conectar'),
                ),
                if (viewModel.connectedChannelId case final channelId?)
                  MetricRowDs(
                    label: 'Canal conectado',
                    value: '${viewModel.connectedDeviceId} · $channelId',
                    mono: true,
                  ),
                const SizedBox(height: 8),
                FilledButton(
                  onPressed: viewModel.isBusy
                      ? null
                      : () => viewModel.runL2capEcho(
                          deviceId: _deviceIdController.text,
                          psm: _psmController.text,
                        ),
                  child: const Text('Eco 1 KB x 20'),
                ),
                if (viewModel.l2capP50Milliseconds case final p50?) ...[
                  MetricRowDs(
                    label: 'Ecos corretos',
                    value: '${viewModel.l2capSuccessCount} de 20',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'p50 (ms)',
                    value: p50.toStringAsFixed(3),
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'p95 (ms)',
                    value:
                        viewModel.l2capP95Milliseconds?.toStringAsFixed(3) ??
                        'Indisponível',
                    mono: true,
                  ),
                ],
              ],
            ),
          ),
          if (viewModel.error case final error?)
            Padding(
              padding: const EdgeInsets.only(top: 16),
              child: Text(
                'Erro: $error',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Theme.of(context).colorScheme.error,
                ),
              ),
            ),
        ],
      ),
    );
  }
}
