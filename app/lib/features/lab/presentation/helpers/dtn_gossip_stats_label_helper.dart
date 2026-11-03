import '../../../../core/dtn/dtn_gossip_stats.dart';

class DtnGossipStatsLabelHelper {
  DtnGossipStatsLabelHelper._();

  static List<({String label, String value})> rowsFor(DtnGossipStats stats) =>
      <({String label, String value})>[
        (label: 'Envelopes guardados', value: '${stats.held}'),
        (label: 'Enviados', value: '${stats.sent}'),
        (label: 'Recebidos (novos)', value: '${stats.received}'),
        (label: 'Duplicados', value: '${stats.duplicates}'),
        (label: 'Rejeitados', value: '${stats.rejected}'),
        (label: 'Snapshots recusados', value: '${stats.snapshotsRefused}'),
        (label: 'Trocas interrompidas', value: '${stats.cutSyncs}'),
        (
          label: 'Versão do snapshot',
          value: stats.snapshotVersion?.toString() ?? 'Nenhum',
        ),
      ];
}
