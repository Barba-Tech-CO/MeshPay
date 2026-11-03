import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/server/client/dio_server_client.dart';
import 'package:meshpay/core/server/client/server_error_code.dart';
import 'package:meshpay/core/server/client/server_exception.dart';
import 'package:meshpay/core/server/messages/server_attestation_level.dart';
import 'package:meshpay/core/server/requests/register_request.dart';

class _StubAdapter implements HttpClientAdapter {
  _StubAdapter(this._respond);

  final ResponseBody Function(RequestOptions options) _respond;
  final List<RequestOptions> requests = <RequestOptions>[];

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    requests.add(options);
    return _respond(options);
  }

  @override
  void close({bool force = false}) {}
}

ResponseBody _body(List<int> bytes, {int status = 200}) =>
    ResponseBody.fromBytes(bytes, status);

DioServerClient _client(_StubAdapter adapter) =>
    DioServerClient(Dio()..httpClientAdapter = adapter);

Future<Object?> _failure(Future<Object?> call) async {
  try {
    await call;
  } on ServerException catch (failure) {
    return failure;
  }
  return null;
}

const String _url = 'http://127.0.0.1:18080';

void main() {
  test('decodes a challenge', () async {
    final nonce = List<int>.filled(32, 7);
    final adapter = _StubAdapter(
      (_) => _body(<int>[0x0a, 32, ...nonce, 0x10, 5]),
    );

    final response = await _client(adapter).getChallenge(_url);

    expect(response.nonce, nonce);
    expect(response.expiresAtUnix, 5);
  });

  test('maps 403 to the typed code without peer text', () async {
    final adapter = _StubAdapter(
      (_) => _body('secret text'.codeUnits, status: 403),
    );

    final failure = await _failure(_client(adapter).getChallenge(_url));

    expect(failure, isA<ServerException>());
    final exception = failure! as ServerException;
    expect(exception.code, ServerErrorCode.forbidden);
    expect(exception.httpStatus, 403);
    expect(exception.toString(), isNot(contains('secret')));
  });

  test('no response maps to unreachable', () async {
    final adapter = _StubAdapter(
      (options) => throw DioException.connectionError(
        requestOptions: options,
        reason: 'down',
      ),
    );

    final failure = await _failure(_client(adapter).getChallenge(_url));

    expect((failure! as ServerException).code, ServerErrorCode.unreachable);
  });

  test('garbage bytes map to malformedResponse', () async {
    final adapter = _StubAdapter((_) => _body(List<int>.filled(8, 0xff)));

    final failure = await _failure(_client(adapter).getChallenge(_url));

    expect(
      (failure! as ServerException).code,
      ServerErrorCode.malformedResponse,
    );
  });

  test('a response over the size cap maps to malformedResponse', () async {
    final adapter = _StubAdapter(
      (_) => _body(List<int>.filled(DioServerClient.maxResponseBytes + 1, 0)),
    );

    final failure = await _failure(_client(adapter).getChallenge(_url));

    expect(
      (failure! as ServerException).code,
      ServerErrorCode.malformedResponse,
    );
  });

  test('posts protobuf content type', () async {
    final adapter = _StubAdapter((_) => _body(<int>[]));

    await _client(adapter).register(
      _url,
      RegisterRequest(
        publicKey: Uint8List(33),
        signature: Uint8List(64),
        attestationLevel: ServerAttestationLevel.labSoftware,
        attestationChain: const <Uint8List>[],
        challenge: Uint8List(0),
      ),
    );

    expect(
      adapter.requests.single.contentType,
      startsWith('application/x-protobuf'),
    );
  });

  test('refuses an insecure url without sending anything', () async {
    final adapter = _StubAdapter((_) => _body(<int>[]));

    final failure = await _failure(
      _client(adapter).getChallenge('http://example.com'),
    );

    expect((failure! as ServerException).code, ServerErrorCode.insecureUrl);
    expect(adapter.requests, isEmpty);
  });
}
