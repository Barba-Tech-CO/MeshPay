import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import '../view_models/channel_lab_view_model.dart';

class ChannelLabPage extends StatelessWidget {
  const ChannelLabPage({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<ChannelLabViewModel>();
    final p50Milliseconds = viewModel.p50Milliseconds;
    final p95Milliseconds = viewModel.p95Milliseconds;

    return Scaffold(
      appBar: AppBar(title: const Text('Canal nativo')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          FilledButton(
            onPressed: viewModel.isBusy ? null : viewModel.runEchoBenchmark,
            child: const Text('Eco do canal nativo (25 KB x 100)'),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Resultado do eco',
            child: Column(
              children: [
                MetricRowDs(
                  label: 'Ecos corretos',
                  value: '${viewModel.successCount} de 100',
                ),
                MetricRowDs(
                  label: 'p50',
                  value: p50Milliseconds == null
                      ? 'Indisponível'
                      : '${p50Milliseconds.toStringAsFixed(3)} ms',
                ),
                MetricRowDs(
                  label: 'p95',
                  value: p95Milliseconds == null
                      ? 'Indisponível'
                      : '${p95Milliseconds.toStringAsFixed(3)} ms',
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
