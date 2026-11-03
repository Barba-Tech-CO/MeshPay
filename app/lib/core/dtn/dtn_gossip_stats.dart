class DtnGossipStats {
  const DtnGossipStats({
    this.held = 0,
    this.sent = 0,
    this.received = 0,
    this.duplicates = 0,
    this.rejected = 0,
    this.snapshotsRefused = 0,
    this.cutSyncs = 0,
    this.snapshotVersion,
  });

  final int held;
  final int sent;

  /// New envelopes stored from the peer.
  final int received;
  final int duplicates;
  final int rejected;

  /// Peer snapshots the buffer refused (invalid, wrong signer or not newer).
  final int snapshotsRefused;

  /// Syncs stopped by the round limit.
  final int cutSyncs;
  final int? snapshotVersion;

  DtnGossipStats copyWith({
    int? held,
    int? sent,
    int? received,
    int? duplicates,
    int? rejected,
    int? snapshotsRefused,
    int? cutSyncs,
    int? snapshotVersion,
  }) => DtnGossipStats(
    held: held ?? this.held,
    sent: sent ?? this.sent,
    received: received ?? this.received,
    duplicates: duplicates ?? this.duplicates,
    rejected: rejected ?? this.rejected,
    snapshotsRefused: snapshotsRefused ?? this.snapshotsRefused,
    cutSyncs: cutSyncs ?? this.cutSyncs,
    snapshotVersion: snapshotVersion ?? this.snapshotVersion,
  );
}
