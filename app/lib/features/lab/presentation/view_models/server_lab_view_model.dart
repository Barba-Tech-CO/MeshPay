import 'package:flutter/foundation.dart';
import 'package:meshpay_native/payment_key_exception.dart';

import '../../../../core/keys/hardware_key_lab_service.dart';
import '../../../../core/keys/payment_key_signer.dart';
import '../../../../core/server/client/server_base_url_helper.dart';
import '../../../../core/server/client/server_client.dart';
import '../../../../core/server/client/server_error_code.dart';
import '../../../../core/server/client/server_exception.dart';
import '../../../../core/server/messages/cheque_stub_body.dart';
import '../../../../core/server/messages/revocation_snapshot_summary.dart';
import '../../../../core/server/messages/server_attestation_level.dart';
import '../../../../core/server/messages/server_key_cert_summary.dart';
import '../../../../core/server/requests/deposit_request.dart';
import '../../../../core/server/requests/issue_request.dart';
import '../../../../core/server/requests/issue_request_body.dart';
import '../../../../core/server/requests/register_request.dart';
import '../../../../core/server/responses/balance_response.dart';
import '../../../../core/server/responses/deposit_response.dart';
import '../../../../core/server/responses/issue_response.dart';
import '../../../../core/server/responses/register_response.dart';
import '../../../../core/server/signing/ecdsa_signature_helper.dart';
import '../../../../core/server/signing/sec1_key_helper.dart';
import '../../../../core/server/signing/server_account_id_helper.dart';
import '../../../../core/server/signing/server_signing_message_helper.dart';
import '../../../../core/server/signing/server_spend_helper.dart';
import '../../../../core/server/trust/server_trust_service.dart';
import '../../../../core/tx/random_bytes_helper.dart';
import '../helpers/server_lab_message_helper.dart';

/// LAB client of the ledger server: registers a Keystore payer key (with or
/// without its attestation chain), issues a chequebook, deposits one cheque to
/// a second lab-registered Keystore key (the payee) and reads signed balances.
/// Account ids are derived locally from the public keys, never taken from the
/// server; the base URL is locked to the one the payer registered against.
class ServerLabViewModel extends ChangeNotifier {
  ServerLabViewModel(
    this._client,
    this._keys,
    this._signer,
    this._trust, {
    this._clock = DateTime.now,
  });

  static const String defaultBaseUrl = 'http://127.0.0.1:18080';
  static const String payerAlias = 'server_lab_payer';
  static const String payeeAlias = 'server_lab_payee';
  static const int chequeCount = 3;
  static const int maxAmountUnits = 100;
  static const int depositAmountUnits = 50;
  static const int _randomIdBytes = 16;
  static const int _labChallengeBytes = 32;

  final ServerClient _client;
  final HardwareKeyLabService _keys;
  final PaymentKeySigner _signer;
  final ServerTrustService _trust;
  final DateTime Function() _clock;

  String _baseUrl = defaultBaseUrl;
  bool _sendAttestation = true;
  bool _isRunning = false;
  String? _error;
  RegisterResponse? _payer;
  RegisterResponse? _payee;
  IssueResponse? _chequebook;
  int _usedStubs = 0;
  DepositResponse? _lastDeposit;
  BalanceResponse? _payerBalance;
  BalanceResponse? _payeeBalance;
  ServerKeyCertSummary? _issuerCert;
  RevocationSnapshotSummary? _snapshot;
  int _counter = 0;
  bool _isDisposed = false;

  String get baseUrl => _baseUrl;
  String get trustBaseUrl => _trust.baseUrl;
  bool get sendAttestation => _sendAttestation;
  bool get isRunning => _isRunning;
  String? get error => _error;
  RegisterResponse? get payer => _payer;
  RegisterResponse? get payee => _payee;
  IssueResponse? get chequebook => _chequebook;
  int get usedStubs => _usedStubs;
  DepositResponse? get lastDeposit => _lastDeposit;
  BalanceResponse? get payerBalance => _payerBalance;
  BalanceResponse? get payeeBalance => _payeeBalance;
  ServerKeyCertSummary? get issuerCert => _issuerCert;
  RevocationSnapshotSummary? get snapshot => _snapshot;

  /// Changing the URL drops every account- and trust-bound state.
  void setBaseUrl(String value) {
    final url = value.trim();
    if (url == _baseUrl) {
      return;
    }
    _resetAccounts();
    _baseUrl = url;
    _notifyIfActive();
  }

  void setSendAttestation(bool value) {
    _sendAttestation = value;
    _notifyIfActive();
  }

  Future<void> register() => _run(() async {
    if (!ServerBaseUrlHelper.isAllowed(_baseUrl)) {
      throw const ServerException(ServerErrorCode.insecureUrl);
    }
    _resetAccounts();
    _payer = await _registerKey(payerAlias, attested: _sendAttestation);
  });

  Future<void> issueChequebook() => _run(() async {
    final account = _payer;
    if (account == null) {
      _error = ServerLabMessageHelper.registerFirst;
      return;
    }
    final body = IssueRequestBody(
      accountId: account.accountId,
      requestId: RandomBytesHelper.secure(_randomIdBytes),
      chequeCount: chequeCount,
      maxAmountUnits: maxAmountUnits,
      ttlSeconds: 3600,
    ).toBytes();
    final signature = await _rawSignature(
      payerAlias,
      ServerSigningMessageHelper.issue(body),
    );
    final book = await _client.issueChequebook(
      _baseUrl,
      IssueRequest(body: body, signature: signature),
    );
    if (book.stubs.length != chequeCount) {
      throw const ServerException(ServerErrorCode.malformedResponse);
    }
    _chequebook = book;
    _usedStubs = 0;
  });

  Future<void> deposit() => _run(() async {
    final book = _chequebook;
    if (book == null || _usedStubs >= book.stubs.length) {
      _error = ServerLabMessageHelper.issueFirst;
      return;
    }
    final receiver = _payee ??= await _registerKey(payeeAlias, attested: false);
    final stub = book.stubs[_usedStubs];
    final counter = _counter + 1;
    final spend = await ServerSpendHelper.build(
      chequeId: ChequeStubBody.fromBytes(stub.body).chequeId,
      payeeAccountId: receiver.accountId,
      amountUnits: depositAmountUnits,
      counter: counter,
      signedAtUnix: _nowUnix(),
      sign: (message) => _rawSignature(payerAlias, message),
    );
    _lastDeposit = await _client.deposit(
      _baseUrl,
      DepositRequest(stub: stub, spend: spend),
    );
    _counter = counter;
    _usedStubs++;
  });

  Future<void> readBalance() => _run(() async {
    final account = _payer;
    if (account == null) {
      _error = ServerLabMessageHelper.registerFirst;
      return;
    }
    _payerBalance = await _balance(payerAlias, account);
    if (_payee case final receiver?) {
      _payeeBalance = await _balance(payeeAlias, receiver);
    }
  });

  /// Exercises the receiver's trust fetch path. A stored cert skips the server
  /// and the root check; a fresh fetch throws when the root is not the pinned one.
  Future<void> fetchTrust() => _run(() async {
    _issuerCert = null;
    _snapshot = null;
    final cert = await _trust.issuerCert();
    final snapshot = await _trust.refreshSnapshot();
    if (snapshot == null) {
      _error = ServerLabMessageHelper.snapshotUnavailable;
      return;
    }
    _issuerCert = ServerKeyCertSummary.fromBytes(cert);
    _snapshot = RevocationSnapshotSummary.fromBytes(snapshot);
  });

  void _resetAccounts() {
    _payer = null;
    _payee = null;
    _chequebook = null;
    _usedStubs = 0;
    _counter = 0;
    _lastDeposit = null;
    _payerBalance = null;
    _payeeBalance = null;
    _issuerCert = null;
    _snapshot = null;
  }

  Future<BalanceResponse> _balance(
    String alias,
    RegisterResponse account,
  ) async {
    final timestamp = _nowUnix();
    final signature = await _rawSignature(
      alias,
      ServerSigningMessageHelper.balance(account.accountId, timestamp),
    );
    return _client.readBalance(
      _baseUrl,
      accountId: account.accountId,
      timestampUnix: timestamp,
      signature: signature,
    );
  }

  Future<RegisterResponse> _registerKey(
    String alias, {
    required bool attested,
  }) async {
    final challenge = attested
        ? (await _client.getChallenge(_baseUrl)).nonce
        : RandomBytesHelper.secure(_labChallengeBytes);
    try {
      await _keys.deleteKey(alias: alias);
    } on PaymentKeyException catch (failure) {
      if (failure.code != 'key_not_found') {
        rethrow;
      }
    }
    final key = await _keys.generateAttestedKey(
      alias: alias,
      challenge: challenge,
      requireStrongBox: false,
    );
    final publicKey = Sec1KeyHelper.compress(key.publicKey);
    final signature = await _rawSignature(
      alias,
      ServerSigningMessageHelper.register(publicKey),
    );
    final response = await _client.register(
      _baseUrl,
      RegisterRequest(
        publicKey: publicKey,
        signature: signature,
        attestationLevel: attested
            ? ServerAttestationLevel.fromSecurityLevel(key.securityLevel)
            : ServerAttestationLevel.labSoftware,
        attestationChain: attested ? key.certificateChain : const <Uint8List>[],
        challenge: attested ? challenge : Uint8List(0),
      ),
    );
    if (!listEquals(
      response.accountId,
      ServerAccountIdHelper.derive(publicKey),
    )) {
      throw const ServerException(ServerErrorCode.malformedResponse);
    }
    return response;
  }

  Future<Uint8List> _rawSignature(String alias, Uint8List message) async =>
      EcdsaSignatureHelper.derToRawLowS(await _signer.sign(alias, message));

  int _nowUnix() => _clock().millisecondsSinceEpoch ~/ 1000;

  Future<void> _run(Future<void> Function() action) async {
    if (_isRunning) {
      return;
    }
    _isRunning = true;
    _error = null;
    _notifyIfActive();
    try {
      await action();
    } on Object catch (failure) {
      _error = ServerLabMessageHelper.messageFor(failure);
    } finally {
      _isRunning = false;
      _notifyIfActive();
    }
  }

  void _notifyIfActive() {
    if (!_isDisposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    super.dispose();
  }
}
