import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../helpers/noise_handshake_label_helper.dart';
import '../helpers/tx_lab_state_label_helper.dart';
import '../view_models/l2cap_receiver_lab_view_model.dart';

class L2capReceiverLabPage extends StatelessWidget {
  const L2capReceiverLabPage({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<L2capReceiverLabViewModel>();
    final statusLabel = switch (viewModel.status) {
      L2capReceiverStatus.closed => 'Fechado',
      L2capReceiverStatus.listening => 'Ouvindo',
      L2capReceiverStatus.connected => 'Conectado',
    };

    final handshakeLabel = NoiseHandshakeLabelHelper.labelFor(
      viewModel.handshakeStatus,
    );

    return Scaffold(
      appBar: AppBar(title: const Text('Receptor L2CAP')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionCardDs(
            title: 'Canal do receptor',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                MetricRowDs(label: 'Estado', value: statusLabel),
                MetricRowDs(
                  label: 'PSM',
                  value: viewModel.psm?.toString() ?? 'Indisponível',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'ID da sessão',
                  value: viewModel.sessionIdHex ?? 'Ainda não gerado',
                  mono: true,
                ),
                MetricRowDs(
                  label: 'Chave pública (hex)',
                  value: viewModel.keyHex ?? 'Ainda não gerada',
                  mono: true,
                ),
                const SizedBox(height: 8),
                FilledButton(
                  onPressed:
                      viewModel.isBusy ||
                          viewModel.status != L2capReceiverStatus.closed ||
                          viewModel.hasActiveOperations
                      ? null
                      : viewModel.start,
                  child: const Text('Iniciar recepção'),
                ),
                OutlinedButton(
                  onPressed: viewModel.isBusy || !viewModel.hasActiveOperations
                      ? null
                      : viewModel.stop,
                  child: const Text('Parar recepção'),
                ),
              ],
            ),
          ),
          if (viewModel.qrPayload case final qrPayload?) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'QR para o pagador',
              child: Center(
                child: QrImageView(
                  data: qrPayload,
                  size: 220,
                  backgroundColor: Colors.white,
                ),
              ),
            ),
          ],
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
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Registro do eco',
            child: MetricRowDs(
              label: 'Mensagens recebidas',
              value: '${viewModel.receivedMessageCount}',
              mono: true,
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Transação (unidade TEST)',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
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
