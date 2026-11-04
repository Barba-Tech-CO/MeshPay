import 'dart:typed_data';

import 'package:meshpay_core/meshpay_core.dart';

/// LAB ONLY: the lab has no issuing server, so the payer seals the
/// `ChequeStub` itself with the core lab ISSUER key. The lab root is public, so
/// receivers pin it only in non-release builds (see `IssuerConfigHelper`);
/// release builds pin the operator root from `--dart-define`.
/// [expirySeconds] is `E` on both sides.
class LabChequeStubHelper {
  LabChequeStubHelper._();

  static const int expirySeconds = 72 * 3600;

  /// Sealed stub for [chequeId] issued to [signerPubkey], expiring `E` after
  /// [issuedAtUnix].
  static Future<Uint8List> forCheque({
    required Uint8List chequeId,
    required Uint8List signerPubkey,
    required int maxAmountUnits,
    required int issuedAtUnix,
  }) => labChequeStub(
    chequeId: chequeId,
    signerPubkey: signerPubkey,
    maxAmountUnits: BigInt.from(maxAmountUnits),
    expiresAtUnix: BigInt.from(issuedAtUnix + expirySeconds),
  );

  static Future<Uint8List> issuerRootPublicKey() => labIssuerRootPublicKey();

  /// Lab ISSUER cert valid from a day ago for 30 days (wall clock; LAB only).
  static Future<Uint8List> issuerCert() => labIssuerCert(
    notBeforeUnix: BigInt.from(
      DateTime.now().millisecondsSinceEpoch ~/ 1000 - 24 * 3600,
    ),
  );
}
