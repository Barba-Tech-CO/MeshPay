import 'dart:typed_data';

import '../requests/deposit_request.dart';
import '../requests/issue_request.dart';
import '../requests/register_request.dart';
import '../responses/balance_response.dart';
import '../responses/challenge_response.dart';
import '../responses/deposit_response.dart';
import '../responses/issue_response.dart';
import '../responses/register_response.dart';
import '../responses/server_key_response.dart';

/// Ledger server API. Failures throw `ServerException`.
abstract interface class ServerClient {
  Future<ChallengeResponse> getChallenge(String baseUrl);

  Future<RegisterResponse> register(String baseUrl, RegisterRequest request);

  Future<IssueResponse> issueChequebook(String baseUrl, IssueRequest request);

  Future<DepositResponse> deposit(String baseUrl, DepositRequest request);

  /// [signature] is raw r||s over the balance message at [timestampUnix].
  Future<BalanceResponse> readBalance(
    String baseUrl, {
    required Uint8List accountId,
    required int timestampUnix,
    required Uint8List signature,
  });

  Future<ServerKeyResponse> getServerKey(String baseUrl);

  /// Raw signed `RevocationSnapshot` bytes, as `installSnapshot` takes them.
  Future<Uint8List> getRevocationSnapshot(String baseUrl);
}
