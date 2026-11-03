import 'dart:typed_data';

import 'package:dio/dio.dart';

import '../../noise/hex_helper.dart';
import '../../routes/api_routes.dart';
import '../requests/deposit_request.dart';
import '../requests/issue_request.dart';
import '../requests/register_request.dart';
import '../responses/balance_response.dart';
import '../responses/challenge_response.dart';
import '../responses/deposit_response.dart';
import '../responses/issue_response.dart';
import '../responses/register_response.dart';
import 'server_base_url_helper.dart';
import 'server_client.dart';
import 'server_error_code.dart';
import 'server_exception.dart';

class DioServerClient implements ServerClient {
  DioServerClient(this._dio);

  static const int maxResponseBytes = 64 * 1024;

  final Dio _dio;

  @override
  Future<ChallengeResponse> getChallenge(String baseUrl) async => _decode(
    await _send('GET', baseUrl + ApiRoutes.challenge),
    ChallengeResponse.fromBytes,
  );

  @override
  Future<RegisterResponse> register(
    String baseUrl,
    RegisterRequest request,
  ) async => _decode(
    await _send('POST', baseUrl + ApiRoutes.accounts, body: request.toBytes()),
    RegisterResponse.fromBytes,
  );

  @override
  Future<IssueResponse> issueChequebook(
    String baseUrl,
    IssueRequest request,
  ) async => _decode(
    await _send(
      'POST',
      baseUrl + ApiRoutes.chequebooks,
      body: request.toBytes(),
    ),
    IssueResponse.fromBytes,
  );

  @override
  Future<DepositResponse> deposit(
    String baseUrl,
    DepositRequest request,
  ) async => _decode(
    await _send('POST', baseUrl + ApiRoutes.deposits, body: request.toBytes()),
    DepositResponse.fromBytes,
  );

  @override
  Future<BalanceResponse> readBalance(
    String baseUrl, {
    required Uint8List accountId,
    required int timestampUnix,
    required Uint8List signature,
  }) async => _decode(
    await _send(
      'GET',
      baseUrl + ApiRoutes.balance(HexHelper.encode(accountId)),
      headers: <String, String>{
        'x-meshpay-timestamp': '$timestampUnix',
        'x-meshpay-signature': HexHelper.encode(signature),
      },
    ),
    BalanceResponse.fromBytes,
  );

  Future<Uint8List> _send(
    String method,
    String url, {
    Uint8List? body,
    Map<String, String>? headers,
  }) async {
    if (!ServerBaseUrlHelper.isAllowed(url)) {
      throw const ServerException(ServerErrorCode.insecureUrl);
    }
    try {
      final response = await _dio.request<List<int>>(
        url,
        data: body,
        options: Options(
          method: method,
          headers: headers,
          contentType: body == null ? null : 'application/x-protobuf',
          responseType: ResponseType.bytes,
        ),
      );
      final data = response.data ?? const <int>[];
      if (data.length > maxResponseBytes) {
        throw const ServerException(ServerErrorCode.malformedResponse);
      }
      return Uint8List.fromList(data);
    } on DioException catch (failure) {
      final status = failure.response?.statusCode;
      if (status == null) {
        throw const ServerException(ServerErrorCode.unreachable);
      }
      throw ServerException(
        ServerErrorCode.fromStatus(status),
        httpStatus: status,
      );
    }
  }

  T _decode<T>(Uint8List bytes, T Function(Uint8List bytes) decode) {
    try {
      return decode(bytes);
    } on Object {
      throw const ServerException(ServerErrorCode.malformedResponse);
    }
  }
}
