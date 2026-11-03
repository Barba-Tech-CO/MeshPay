import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import '../../../../core/noise/hex_helper.dart';
import '../helpers/payment_key_message_helper.dart';
import '../view_models/hardware_key_lab_view_model.dart';

class HardwareKeyLabPage extends StatelessWidget {
  const HardwareKeyLabPage({super.key});

  static const int _shortKeyChars = 16;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<HardwareKeyLabViewModel>();
    final result = viewModel.result;

    return Scaffold(
      appBar: AppBar(title: const Text('Chave de hardware')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SwitchListTile(
            title: const Text('Exigir StrongBox'),
            value: viewModel.requireStrongBox,
            onChanged: viewModel.isRunning
                ? null
                : viewModel.setRequireStrongBox,
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: viewModel.isRunning ? null : viewModel.runProbe,
            child: const Text('Gerar e testar chave'),
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
          if (result != null) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'Resultado',
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Nível de segurança',
                    value: PaymentKeyMessageHelper.levelLabel(
                      result.securityLevel,
                    ),
                  ),
                  MetricRowDs(
                    label: 'StrongBox no aparelho',
                    value: result.hasStrongBox ? 'Sim' : 'Não',
                  ),
                  MetricRowDs(
                    label: 'API do Android',
                    value: '${result.apiLevel}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Certificados na atestação',
                    value: '${result.chainLength}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Chave pública',
                    value:
                        '${HexHelper.encode(result.publicKey).substring(0, _shortKeyChars)}...',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Assinatura gerada',
                    value: '${result.signatureLength} bytes DER',
                    mono: true,
                  ),
                  const MetricRowDs(label: 'Chave de teste', value: 'Apagada'),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
