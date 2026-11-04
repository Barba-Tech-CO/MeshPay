import 'package:design_system/design_system.dart';
import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import '../../../../core/noise/hex_helper.dart';
import '../helpers/server_lab_message_helper.dart';
import '../view_models/server_lab_view_model.dart';

class ServerLabPage extends StatelessWidget {
  const ServerLabPage({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<ServerLabViewModel>();
    final busy = viewModel.isRunning;
    final payer = viewModel.payer;
    final payee = viewModel.payee;
    final chequebook = viewModel.chequebook;
    final deposit = viewModel.lastDeposit;
    final payerBalance = viewModel.payerBalance;
    final payeeBalance = viewModel.payeeBalance;
    final issuerCert = viewModel.issuerCert;
    final snapshot = viewModel.snapshot;

    return Scaffold(
      appBar: AppBar(title: const Text('Servidor do ledger')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextFormField(
            initialValue: viewModel.baseUrl,
            enabled: !busy,
            decoration: const InputDecoration(labelText: 'URL do servidor'),
            keyboardType: TextInputType.url,
            onChanged: viewModel.setBaseUrl,
          ),
          SwitchListTile(
            title: const Text('Enviar cadeia de atestação'),
            subtitle: Text(
              viewModel.sendAttestation
                  ? 'Ligado: registro atestado'
                  : 'Desligado: registro de laboratório',
            ),
            value: viewModel.sendAttestation,
            onChanged: busy ? null : viewModel.setSendAttestation,
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              FilledButton(
                onPressed: busy ? null : viewModel.register,
                child: const Text('Registrar'),
              ),
              FilledButton(
                onPressed: busy ? null : viewModel.issueChequebook,
                child: const Text('Emitir talão'),
              ),
              FilledButton(
                onPressed: busy ? null : viewModel.deposit,
                child: const Text('Depositar'),
              ),
              FilledButton(
                onPressed: busy ? null : viewModel.readBalance,
                child: const Text('Ler saldo'),
              ),
              FilledButton(
                onPressed: busy ? null : viewModel.fetchTrust,
                child: const Text('Buscar confiança'),
              ),
            ],
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
          if (issuerCert != null && snapshot != null) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'Confiança do servidor',
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Servidor',
                    value: viewModel.trustBaseUrl,
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Chave do emissor',
                    value: HexHelper.encode(issuerCert.keyId),
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Certificado válido até',
                    value: ServerLabMessageHelper.unixToText(
                      issuerCert.notAfterUnix,
                    ),
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Snapshot emitido em',
                    value: ServerLabMessageHelper.unixToText(
                      snapshot.issuedAtUnix,
                    ),
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Chaves de servidor revogadas',
                    value: '${snapshot.revokedServerKeyCount}',
                    mono: true,
                  ),
                ],
              ),
            ),
          ],
          if (payer != null) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'Conta pagadora',
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Conta',
                    value: HexHelper.encode(payer.accountId),
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Época',
                    value: '${payer.epoch}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Criada agora',
                    value: ServerLabMessageHelper.yesNo(payer.created),
                  ),
                  MetricRowDs(
                    label: 'Disponível no registro',
                    value: '${payer.availableUnits}',
                    mono: true,
                  ),
                ],
              ),
            ),
          ],
          if (chequebook != null) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'Talão',
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Talão',
                    value: HexHelper.encode(chequebook.chequebookId),
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Cheques usados',
                    value: '${viewModel.usedStubs}/${chequebook.stubs.length}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Reserva',
                    value: '${chequebook.reserveUnits}',
                    mono: true,
                  ),
                ],
              ),
            ),
          ],
          if (deposit != null) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'Último depósito',
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Status',
                    value: ServerLabMessageHelper.redeemStatusLabel(
                      deposit.status,
                    ),
                  ),
                  MetricRowDs(
                    label: 'Creditado',
                    value: '${deposit.creditedUnits}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Prova de equivocação',
                    value: ServerLabMessageHelper.yesNo(deposit.hasProof),
                  ),
                ],
              ),
            ),
          ],
          if (payerBalance != null) ...[
            const SizedBox(height: 16),
            SectionCardDs(
              title: 'Saldos (leitura assinada)',
              child: Column(
                children: [
                  MetricRowDs(
                    label: 'Pagador disponível',
                    value: '${payerBalance.availableUnits}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Pagador reservado',
                    value: '${payerBalance.reservedUnits}',
                    mono: true,
                  ),
                  MetricRowDs(
                    label: 'Pagador congelado',
                    value: ServerLabMessageHelper.yesNo(payerBalance.frozen),
                  ),
                  if (payee != null && payeeBalance != null)
                    MetricRowDs(
                      label: 'Recebedor disponível',
                      value: '${payeeBalance.availableUnits}',
                      mono: true,
                    ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
