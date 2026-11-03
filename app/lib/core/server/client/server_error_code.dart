enum ServerErrorCode {
  badRequest,
  forbidden,
  notFound,
  unprocessable,
  tooManyRequests,
  internal,
  unavailable,
  unexpectedStatus,
  unreachable,
  malformedResponse,
  insecureUrl;

  static ServerErrorCode fromStatus(int status) => switch (status) {
    400 => badRequest,
    403 => forbidden,
    404 => notFound,
    422 => unprocessable,
    429 => tooManyRequests,
    500 => internal,
    503 => unavailable,
    _ => unexpectedStatus,
  };
}
