/// Fixed codes sent in error frames; never raw exception text.
enum TxErrorCode {
  invalidMessage,
  invalidKey,
  helloRepeated,
  helloMissing,
  notAddressedToUs,
  badSignature,
  txIdMismatch,
  conflict,
  tooLarge,
  notCommittable,
  internal,
  dtnSummaryRepeated,
  dtnRoundLimit,
  localKeyInconsistent;

  int get wire => index + 1;

  static TxErrorCode fromWire(int wire) =>
      wire >= 1 && wire <= values.length ? values[wire - 1] : internal;
}
