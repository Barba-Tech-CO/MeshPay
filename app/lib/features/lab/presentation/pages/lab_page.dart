import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import '../../../../core/routes/app_routes.dart';

class LabPage extends StatelessWidget {
  const LabPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Laboratório')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionCardDs(
            title: 'Medições da Fase 0',
            child: Column(
              children: [
                ListTile(
                  title: const Text('Relógio e reinício'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => context.pushNamed(AppRoutes.labClock),
                ),
                ListTile(
                  title: const Text('Canal nativo'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => context.pushNamed(AppRoutes.labChannel),
                ),
                ListTile(
                  title: const Text('Bluetooth'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => context.pushNamed(AppRoutes.labBluetooth),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Canal L2CAP (Fase 1)',
            child: Column(
              children: [
                ListTile(
                  title: const Text('Receptor L2CAP'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => context.pushNamed(AppRoutes.labL2capReceiver),
                ),
                ListTile(
                  title: const Text('Pagador L2CAP'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => context.pushNamed(AppRoutes.labL2capPayer),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Chaves de hardware',
            child: ListTile(
              title: const Text('Chave de hardware'),
              trailing: const Icon(Icons.chevron_right),
              onTap: () => context.pushNamed(AppRoutes.labHardwareKey),
            ),
          ),
          const SizedBox(height: 16),
          SectionCardDs(
            title: 'Servidor (Fase 4)',
            child: ListTile(
              title: const Text('Servidor do ledger'),
              trailing: const Icon(Icons.chevron_right),
              onTap: () => context.pushNamed(AppRoutes.labServer),
            ),
          ),
        ],
      ),
    );
  }
}
