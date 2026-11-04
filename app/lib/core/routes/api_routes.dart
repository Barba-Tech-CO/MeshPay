/// Ledger server endpoints (server/src/http/router.rs).
class ApiRoutes {
  static const String challenge = '/v1/challenge';
  static const String accounts = '/v1/accounts';
  static const String chequebooks = '/v1/chequebooks';
  static const String deposits = '/v1/deposits';
  static const String serverKey = '/v1/server-key';
  static const String revocationSnapshot = '/v1/revocation-snapshot';

  static String balance(String accountIdHex) =>
      '/v1/accounts/$accountIdHex/balance';
}
