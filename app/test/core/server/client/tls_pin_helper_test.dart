import 'dart:convert';
import 'dart:io';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/server/client/server_base_url_helper.dart';
import 'package:meshpay/core/server/client/tls_pin_helper.dart';

final String? _opensslSkipReason = _hasOpenSsl()
    ? null
    : 'openssl is not on PATH; runtime TLS certificate test skipped.';

bool _hasOpenSsl() {
  try {
    return Process.runSync('openssl', <String>['version']).exitCode == 0;
  } on ProcessException {
    return false;
  }
}

Future<void> _runOpenSsl(Directory directory, List<String> arguments) async {
  final result = await Process.run(
    'openssl',
    arguments,
    workingDirectory: directory.path,
  );
  if (result.exitCode != 0) {
    throw StateError('openssl ${arguments.first} failed: ${result.stderr}');
  }
}

Future<void> _createSignedServerCertificate(
  Directory directory,
  String name,
) async {
  final caKey = 'ca-$name.key';
  final caPem = 'ca-$name.pem';
  final serverKey = 'server-$name.key';
  final serverCsr = 'server-$name.csr';
  final serverPem = 'server-$name.pem';
  await _runOpenSsl(directory, <String>[
    'req',
    '-x509',
    '-newkey',
    'rsa:2048',
    '-nodes',
    '-keyout',
    caKey,
    '-out',
    caPem,
    '-days',
    '1',
    '-subj',
    '/CN=MeshPay Test CA $name',
    '-addext',
    'basicConstraints=critical,CA:TRUE',
    '-addext',
    'keyUsage=critical,keyCertSign,cRLSign',
  ]);
  await _runOpenSsl(directory, <String>[
    'genpkey',
    '-algorithm',
    'RSA',
    '-pkeyopt',
    'rsa_keygen_bits:2048',
    '-out',
    serverKey,
  ]);
  await _runOpenSsl(directory, <String>[
    'req',
    '-new',
    '-key',
    serverKey,
    '-out',
    serverCsr,
    '-subj',
    '/CN=127.0.0.1',
  ]);
  await File('${directory.path}/server.ext').writeAsString(
    'subjectAltName=IP:127.0.0.1\n'
    'extendedKeyUsage=serverAuth\n'
    'keyUsage=digitalSignature,keyEncipherment\n'
    'basicConstraints=critical,CA:FALSE\n',
  );
  await _runOpenSsl(directory, <String>[
    'x509',
    '-req',
    '-in',
    serverCsr,
    '-CA',
    caPem,
    '-CAkey',
    caKey,
    '-CAcreateserial',
    '-out',
    serverPem,
    '-days',
    '1',
    '-sha256',
    '-extfile',
    'server.ext',
  ]);
  await File('${directory.path}/chain-$name.pem').writeAsString(
    '${await File('${directory.path}/$serverPem').readAsString()}\n'
    '${await File('${directory.path}/$caPem').readAsString()}',
  );
}

Future<HttpServer> _startTlsServer(
  Directory directory,
  String chainName,
  String keyName,
) async {
  final context = SecurityContext()
    ..useCertificateChain('${directory.path}/chain-$chainName.pem')
    ..usePrivateKey('${directory.path}/$keyName');
  final server = await HttpServer.bindSecure(
    InternetAddress.loopbackIPv4,
    0,
    context,
  );
  server.listen((request) async {
    request.response.statusCode = HttpStatus.ok;
    await request.response.close();
  });
  return server;
}

void main() {
  test('release configuration requires an HTTPS URL and a pinned CA', () {
    expect(
      () => TlsPinHelper.configuredAdapter(
        isRelease: true,
        configuredBaseUrl: 'https://ledger.example',
        configuredCaPemB64: '',
      ),
      throwsStateError,
    );
    expect(
      () => TlsPinHelper.configuredAdapter(
        isRelease: true,
        configuredBaseUrl: 'http://127.0.0.1:18080',
        configuredCaPemB64: base64.encode(<int>[1]),
      ),
      throwsStateError,
    );
    expect(
      () => TlsPinHelper.configuredAdapter(
        isRelease: true,
        configuredBaseUrl: 'https://ledger.example',
        configuredCaPemB64: 'not base64!',
      ),
      throwsStateError,
    );
  });

  test('debug without a CA keeps the loopback HTTP lab URL available', () {
    expect(
      TlsPinHelper.configuredAdapter(
        isRelease: false,
        configuredBaseUrl: 'http://127.0.0.1:18080',
        configuredCaPemB64: '',
      ),
      isNull,
    );
    expect(
      ServerBaseUrlHelper.isAllowed(
        'http://127.0.0.1:18080',
        isRelease: false,
      ),
      isTrue,
    );
    expect(
      ServerBaseUrlHelper.isAllowed(
        'http://127.0.0.1:18080',
        isRelease: true,
      ),
      isFalse,
    );
  });

  test(
    'accepts the pinned CA and rejects other and self-signed certificates',
    () async {
      final directory = Directory.systemTemp.createTempSync('meshpay-tls-pin');
      final servers = <HttpServer>[];
      Dio? dio;
      try {
        await _createSignedServerCertificate(directory, 'a');
        await _createSignedServerCertificate(directory, 'b');
        await _runOpenSsl(directory, <String>[
          'req',
          '-x509',
          '-newkey',
          'rsa:2048',
          '-nodes',
          '-keyout',
          'self.key',
          '-out',
          'self.pem',
          '-days',
          '1',
          '-subj',
          '/CN=127.0.0.1',
          '-addext',
          'subjectAltName=IP:127.0.0.1',
          '-addext',
          'extendedKeyUsage=serverAuth',
          '-addext',
          'keyUsage=digitalSignature,keyEncipherment',
          '-addext',
          'basicConstraints=critical,CA:FALSE',
        ]);
        await File('${directory.path}/chain-self.pem').writeAsString(
          await File('${directory.path}/self.pem').readAsString(),
        );

        servers
          ..add(await _startTlsServer(directory, 'a', 'server-a.key'))
          ..add(await _startTlsServer(directory, 'b', 'server-b.key'))
          ..add(await _startTlsServer(directory, 'self', 'self.key'));
        final caPem = await File('${directory.path}/ca-a.pem').readAsBytes();
        final adapter = TlsPinHelper.configuredAdapter(
          isRelease: false,
          configuredBaseUrl: 'https://127.0.0.1:${servers[0].port}',
          configuredCaPemB64: base64.encode(caPem),
        );
        if (adapter == null) {
          throw StateError('Configured CA did not create a TLS adapter.');
        }
        dio = Dio(
          BaseOptions(
            connectTimeout: const Duration(seconds: 5),
            receiveTimeout: const Duration(seconds: 5),
          ),
        )..httpClientAdapter = adapter;

        final trustedResponse = await dio.get<int>(
          'https://127.0.0.1:${servers[0].port}',
        );
        expect(trustedResponse.statusCode, HttpStatus.ok);

        for (final server in servers.skip(1)) {
          await expectLater(
            dio.get<int>('https://127.0.0.1:${server.port}'),
            throwsA(
              isA<DioException>().having(
                (failure) => failure.error,
                'TLS error',
                isA<TlsException>(),
              ),
            ),
          );
        }
      } finally {
        dio?.close(force: true);
        for (final server in servers) {
          await server.close(force: true);
        }
        directory.deleteSync(recursive: true);
      }
    },
    skip: _opensslSkipReason,
  );
}
