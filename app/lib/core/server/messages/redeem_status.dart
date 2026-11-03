/// Mirror of server/src/wire/redeem_status.rs.
enum RedeemStatus {
  unspecified(0),
  paid(1),
  alreadyRedeemedSameBody(2),
  rejectedConflict(3),
  rejectedExpired(4),
  rejectedRevoked(5),
  rejectedInvalid(6);

  const RedeemStatus(this.value);

  final int value;

  static RedeemStatus fromValue(int value) => RedeemStatus.values.firstWhere(
    (status) => status.value == value,
    orElse: () => RedeemStatus.unspecified,
  );
}
