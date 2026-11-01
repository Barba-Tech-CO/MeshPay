import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import '../view_models/clock_lab_view_model.dart';

class ClockLabPage extends StatefulWidget {
  const ClockLabPage({super.key});

  @override
  State<ClockLabPage> createState() => _ClockLabPageState();
}

class _ClockLabPageState extends State<ClockLabPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        context.read<ClockLabViewModel>().initialize();
      }
    });
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<ClockLabViewModel>();
    final latestEntry = viewModel.latestEntry;

    return Scaffold(
      appBar: AppBar(title: const Text('Relógio e reinício')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionCardDs(
            title: 'Amostra mais recente',
            child: latestEntry == null
                ? const Text('Nenhuma amostra registrada.')
                : Column(
                    children: [
                      MetricRowDs(
                        label: 'Sequência',
                        value: '${latestEntry.sequence}',
                        mono: true,
                      ),
                      MetricRowDs(
                        label: 'Relógio monotônico (ns)',
                        value: '${latestEntry.elapsedRealtimeNanos}',
                        mono: true,
                      ),
                      MetricRowDs(
                        label: 'Contagem de reinícios',
                        value:
                            latestEntry.bootCount?.toString() ?? 'Indisponível',
                        mono: true,
                      ),
                      MetricRowDs(
                        label: 'Rótulo',
                        value: latestEntry.label,
                      ),
                    ],
                  ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: viewModel.isLoading ? null : viewModel.readSample,
            child: const Text('Ler relógio'),
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
          const SizedBox(height: 16),
          Text(
            'Registro completo',
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const SizedBox(height: 8),
          for (final entry in viewModel.entries)
            SectionCardDs(
              title: entry.label,
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Sequência',
                    value: '${entry.sequence}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Relógio monotônico (ns)',
                    value: '${entry.elapsedRealtimeNanos}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Contagem de reinícios',
                    value: entry.bootCount?.toString() ?? 'Indisponível',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Marcador de início do app',
                    value: entry.isAppLaunch ? 'Sim' : 'Não',
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}
