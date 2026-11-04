import 'dart:typed_data';

import 'package:meshpay/core/server/client/server_client.dart';
import 'package:meshpay/core/server/messages/redeem_status.dart';
import 'package:meshpay/core/server/requests/deposit_request.dart';
import 'package:meshpay/core/server/requests/issue_request.dart';
import 'package:meshpay/core/server/requests/register_request.dart';
import 'package:meshpay/core/server/responses/balance_response.dart';
import 'package:meshpay/core/server/responses/challenge_response.dart';
import 'package:meshpay/core/server/responses/deposit_response.dart';
import 'package:meshpay/core/server/responses/issue_response.dart';
import 'package:meshpay/core/server/responses/register_response.dart';
import 'package:meshpay/core/server/responses/server_key_response.dart';
import 'package:meshpay/core/server/signing/server_account_id_helper.dart';

class FakeServerClient implements ServerClient {
  final List<String> calls = <String>[];
  final List<RegisterRequest> registers = <RegisterRequest>[];
  final List<IssueRequest> issues = <IssueRequest>[];
  final List<DepositRequest> deposits = <DepositRequest>[];
  final Uint8List nonce = Uint8List.fromList(List<int>.filled(32, 0x0c));
  IssueResponse? issueResponse;
  Object? registerError;
  Uint8List? accountIdOverride;
  ServerKeyResponse? serverKey;
  Uint8List? snapshot;
  Object? trustError;

  @override
  Future<ChallengeResponse> getChallenge(String baseUrl) async {
    calls.add('challenge:$baseUrl');
    return ChallengeResponse(nonce: nonce, expiresAtUnix: 1);
  }

  @override
  Future<RegisterResponse> register(
    String baseUrl,
    RegisterRequest request,
  ) async {
    calls.add('register');
    registers.add(request);
    if (registerError case final error?) {
      throw error;
    }
    return RegisterResponse(
      accountId:
          accountIdOverride ?? ServerAccountIdHelper.derive(request.publicKey),
      epoch: 1,
      created: true,
      availableUnits: 10000,
    );
  }

  @override
  Future<IssueResponse> issueChequebook(
    String baseUrl,
    IssueRequest request,
  ) async {
    calls.add('issue');
    issues.add(request);
    return issueResponse!;
  }

  @override
  Future<DepositResponse> deposit(
    String baseUrl,
    DepositRequest request,
  ) async {
    calls.add('deposit');
    deposits.add(request);
    return const DepositResponse(
      status: RedeemStatus.paid,
      hasProof: false,
      creditedUnits: 50,
    );
  }

  @override
  Future<BalanceResponse> readBalance(
    String baseUrl, {
    required Uint8List accountId,
    required int timestampUnix,
    required Uint8List signature,
  }) async {
    calls.add('balance:${accountId.first}');
    return BalanceResponse(
      availableUnits: accountId.first,
      reservedUnits: 0,
      frozen: false,
    );
  }

  @override
  Future<ServerKeyResponse> getServerKey(String baseUrl) async {
    calls.add('serverKey');
    if (trustError case final error?) {
      throw error;
    }
    return serverKey!;
  }

  @override
  Future<Uint8List> getRevocationSnapshot(String baseUrl) async {
    calls.add('snapshot');
    if (trustError case final error?) {
      throw error;
    }
    return snapshot!;
  }
}
