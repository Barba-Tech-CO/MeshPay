import 'server_error_code.dart';

/// Typed server failure. The response body (server free text) is never kept.
class ServerException implements Exception {
  const ServerException(this.code, {this.httpStatus});

  final ServerErrorCode code;
  final int? httpStatus;

  @override
  String toString() => 'ServerException(${code.name}, $httpStatus)';
}
