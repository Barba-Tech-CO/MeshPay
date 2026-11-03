import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

import '../../../../core/tx/payer_tx_lab_flow.dart';
import '../helpers/dtn_gossip_stats_label_helper.dart';
import '../helpers/noise_handshake_label_helper.dart';
import '../helpers/tx_lab_state_label_helper.dart';
import '../view_models/l2cap_payer_lab_view_model.dart';

class L2capPayerLabPage extends StatefulWidget {
  const L2capPayerLabPage({super.key});

  @override
  State<L2capPayerLabPage> createState() => _L2capPayerLabPageState();
}

class _L2capPayerLabPageState extends State<L2capPayerLabPage> {
  final TextEditingController _sessionController = TextEditingController();
  final TextEditingController _psmController = TextEditingController();
  final TextEditingController _keyController = TextEditingController();
  final TextEditingController _deviceIdController = TextEditingController();
  bool _isQrScannerVisible = false;

  @override
  void dispose() {
    _sessionController.dispose();
    _psmController.dispose();
    _keyController.dispose();
    _deviceIdController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<L2capPayerLabViewModel>();
    final handshakeLabel = NoiseHandshakeLabelHelper.labelFor(
      viewModel.handshakeStatus,
    );

    return Scaffold(
      appBar: AppBar(title: const Text('Pagador L2CAP')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionCardDs(
            title: 'Dados do receptor',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextField(
                  controller: _sessionController,
                  onChanged: viewModel.updateSessionIdHex,
                  decoration: const InputDecoration(
                    labelText: 'ID da sessão (hex)',
                  ),
                ),
                TextField(
                  controller: _psmController,
                  onChanged: viewModel.updatePsm,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(labelText: 'PSM'),
                ),
                TextField(
                  controller: _keyController,
                  onChanged: viewModel.updateKeyHex,
                  decoration: const InputDecoration(
                    labelText: 'Chave pública do receptor (hex)',
                  ),
                ),
                const SizedBox(height: 8),
                OutlinedButton(
                  onPressed: () {
                    setState(() {
                      _isQrScannerVisible = !_isQrScannerVisible;
                    });
                  },
                  child: Text(
                    _isQrScannerVisible
                        ? 'Fechar leitor de QR'
                        : 'Ler QR do receptor',
                  ),
                ),
                if (_isQrScannerVisible)
                  SizedBox(
                    height: 280,
                    child: MobileScanner(
                      onDetect: (capture) =>
                          _handleQrCapture(capture, viewModel),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Busca do dispositivo',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextField(
                  controller: _deviceIdController,
                  onChanged: viewModel.updateDeviceId,
                  decoration: const InputDecoration(
                    labelText: 'ID do dispositivo',
                  ),
                ),
                const SizedBox(height: 8),
                FilledButton(
                  onPressed: viewModel.isScanning || viewModel.isBusy
                      ? null
                      : viewModel.startScan,
                  child: const Text('Buscar sessão'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isScanning ? viewModel.stopScan : null,
                  child: const Text('Parar busca'),
                ),
                if (viewModel.scanResults.isEmpty)
                  const Text('Nenhum dispositivo encontrado.')
                else
                  for (final result in viewModel.scanResults)
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      title: Text(result.deviceId),
                      subtitle: Text(
                        'Sessão ${result.sessionIdHex} · RSSI ${result.rssi} dBm',
                      ),
                      onTap: () {
                        _deviceIdController.text = result.deviceId;
                        viewModel.selectDevice(result.deviceId);
                      },
                    ),
                FilledButton(
                  onPressed: viewModel.isBusy || viewModel.isConnected
                      ? null
                      : viewModel.connect,
                  child: const Text('Conectar'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isConnected ? viewModel.close : null,
                  child: const Text('Fechar conexão'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Handshake Noise XK',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                MetricRowDs(label: 'Estado', value: handshakeLabel),
                MetricRowDs(
                  label: 'Mensagens (bytes)',
                  value: viewModel.handshakeMessageSizes.isEmpty
                      ? 'Indisponível'
                      : viewModel.handshakeMessageSizes.join(' / '),
                  mono: true,
                ),
                MetricRowDs(
                  label: 'Total (esperado 160)',
                  value: viewModel.handshakeMessageSizes.isEmpty
                      ? 'Indisponível'
                      : '${viewModel.handshakeTotalBytes}',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'Handshake p50 (ms)',
                  value:
                      viewModel.handshakeP50Milliseconds?.toStringAsFixed(3) ??
                      'Indisponível',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'Handshake p95 (ms)',
                  value:
                      viewModel.handshakeP95Milliseconds?.toStringAsFixed(3) ??
                      'Indisponível',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'Conexões medidas',
                  value: '${viewModel.handshakeDurationsMilliseconds.length}',
                  mono: true,
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Eco cifrado (25 KB x 5)',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                FilledButton(
                  onPressed: viewModel.isConnected && !viewModel.isBusy
                      ? viewModel.runEchoBenchmark
                      : null,
                  child: const Text('Executar eco'),
                ),
                MetricRowDs(
                  label: 'Ecos corretos',
                  value: '${viewModel.successCount} de 5',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'p50 (ms)',
                  value:
                      viewModel.p50Milliseconds?.toStringAsFixed(3) ??
                      'Indisponível',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'p95 (ms)',
                  value:
                      viewModel.p95Milliseconds?.toStringAsFixed(3) ??
                      'Indisponível',
                  mono: true,
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Cheque de teste (unidade TEST)',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                FilledButton(
                  onPressed: viewModel.isConnected && !viewModel.isBusy
                      ? viewModel.sendTestCheque
                      : null,
                  child: const Text('Enviar cheque de teste'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isConnected && !viewModel.isBusy
                      ? viewModel.resendPending
                      : null,
                  child: const Text('Reenviar pendente'),
                ),
                MetricRowDs(
                  label: 'Valor',
                  value:
                      '${PayerTxLabFlow.testAmountUnits} '
                      '${PayerTxLabFlow.testUnit}',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'Estado',
                  value: TxLabStateLabelHelper.labelFor(viewModel.txState),
                ),
                MetricRowDs(
                  label: 'ID da transação',
                  value: viewModel.txIdHex ?? 'Indisponível',
                  mono: true,
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Gossip DTN',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                FilledButton(
                  onPressed: viewModel.isBusy
                      ? null
                      : viewModel.createTestEnvelope,
                  child: const Text('Criar envelope de teste'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isConnected && !viewModel.isBusy
                      ? viewModel.syncDtn
                      : null,
                  child: const Text('Sincronizar (gossip)'),
                ),
                for (final row in DtnGossipStatsLabelHelper.rowsFor(
                  viewModel.dtnStats,
                ))
                  MetricRowDs(
                    label: row.label,
                    value: row.value,
                    mono: true,
                  ),
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

  void _handleQrCapture(
    BarcodeCapture capture,
    L2capPayerLabViewModel viewModel,
  ) {
    if (capture.barcodes.isEmpty) {
      return;
    }
    final payload = capture.barcodes.first.rawValue;
    if (payload == null || !viewModel.handleQrPayload(payload) || !mounted) {
      return;
    }
    setState(() {
      _sessionController.text = viewModel.sessionIdHex;
      _psmController.text = viewModel.psm;
      _keyController.text = viewModel.keyHex;
      _isQrScannerVisible = false;
    });
  }
}
