import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/meshpay_native.dart';
import 'package:meshpay_native/native_protocol_exception.dart';
import 'package:meshpay_native/payment_key_exception.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const methodChannel = MethodChannel('meshpay_native/methods');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[];
  late MeshpayNative plugin;
  Object? Function(MethodCall call) reply = (_) => null;

  setUp(() {
    calls.clear();
    reply = (_) => null;
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      calls.add(call);
      return reply(call);
    });
    plugin = MeshpayNative(methodChannel: methodChannel);
  });

  tearDown(() => messenger.setMockMethodCallHandler(methodChannel, null));

  test(
    'generateAttestedPaymentKey sends arguments and decodes the key',
    () async {
      final point = Uint8List(65)..[0] = 4;
      reply = (_) => <String, Object>{
        'publicKey': point,
        'certificateChain': <Uint8List>[
          Uint8List.fromList(<int>[1]),
          Uint8List.fromList(<int>[2]),
        ],
        'securityLevel': 'strongBox',
      };

      final key = await plugin.generateAttestedPaymentKey(
        alias: 'k1',
        challenge: Uint8List.fromList(<int>[9]),
        requireStrongBox: true,
        singleUse: true,
      );

      expect(calls.single.method, 'generateAttestedPaymentKey');
      expect(calls.single.arguments, <String, Object>{
        'alias': 'k1',
        'challenge': Uint8List.fromList(<int>[9]),
        'requireStrongBox': true,
        'singleUse': true,
      });
      expect(key.publicKey.length, 65);
      expect(key.certificateChain.length, 2);
      expect(key.securityLevel, KeySecurityLevel.strongBox);
    },
  );

  test('maps every native error code to PaymentKeyException', () async {
    const codes = <String>[
      'strongbox_unavailable',
      'alias_exists',
      'invalid_alias',
      'invalid_count',
      'data_too_large',
      'attestation_unavailable',
      'max_usage_count_unsupported',
    ];
    String? code;
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      throw PlatformException(code: code!, message: 'm');
    });

    final calls = <Future<Object?> Function()>[
      () => plugin.generateAttestedPaymentKey(
        alias: 'k1',
        challenge: Uint8List(1),
        requireStrongBox: true,
      ),
      () => plugin.signWithPaymentKey(alias: 'k1', data: Uint8List(1)),
      () => plugin.deletePaymentKey(alias: 'k1'),
      () => plugin.getPaymentKeyInfo(alias: 'k1'),
      () => plugin.getDeviceCapabilities(),
      () => plugin.generateSingleUseKeys(
        aliasPrefix: 'p',
        count: 1,
        requireStrongBox: false,
      ),
    ];
    for (final native in codes) {
      code = native;
      for (final call in calls) {
        await expectLater(
          call(),
          throwsA(
            isA<PaymentKeyException>()
                .having((PaymentKeyException e) => e.code, 'code', native)
                .having((PaymentKeyException e) => e.message, 'message', 'm'),
          ),
        );
      }
    }
  });

  test('sign, delete and keyInfo use the expected methods', () async {
    reply = (MethodCall call) => switch (call.method) {
      'signWithPaymentKey' => Uint8List.fromList(<int>[0x30, 1]),
      'paymentKeyInfo' => <String, Object>{
        'securityLevel': 'tee',
        'isHardwareBacked': true,
      },
      _ => null,
    };

    final signature = await plugin.signWithPaymentKey(
      alias: 'k1',
      data: Uint8List.fromList(<int>[1, 2]),
    );
    await plugin.deletePaymentKey(alias: 'k1');
    final info = await plugin.getPaymentKeyInfo(alias: 'k1');

    expect(signature.first, 0x30);
    expect(info.securityLevel, KeySecurityLevel.tee);
    expect(info.isHardwareBacked, isTrue);
    expect(calls.map((MethodCall c) => c.method), <String>[
      'signWithPaymentKey',
      'deletePaymentKey',
      'paymentKeyInfo',
    ]);
  });

  test('decodes device capabilities', () async {
    reply = (_) => <String, Object>{
      'hasStrongBox': false,
      'apiLevel': 29,
      'supportsMaxUsageCount': false,
      'hasSingleUseKeyFeature': false,
      'hasLimitedUseKeyFeature': false,
    };

    final capabilities = await plugin.getDeviceCapabilities();

    expect(capabilities.hasStrongBox, isFalse);
    expect(capabilities.apiLevel, 29);
    expect(capabilities.supportsMaxUsageCount, isFalse);
  });

  test(
    'generateSingleUseKeys decodes durations and the unsupported flag',
    () async {
      reply = (_) => <String, Object>{
        'aliases': <String>['p0', 'p1'],
        'durationsMs': <int>[120, 95],
        'maxUsageCountApplied': false,
      };

      final batch = await plugin.generateSingleUseKeys(
        aliasPrefix: 'p',
        count: 2,
        requireStrongBox: false,
      );

      expect(calls.single.arguments, <String, Object?>{
        'aliasPrefix': 'p',
        'count': 2,
        'requireStrongBox': false,
        'challenge': null,
        'applyMaxUsageCount': true,
      });
      expect(batch.durationsMs, <int>[120, 95]);
      expect(batch.maxUsageCountApplied, isFalse);
    },
  );

  test('rejects malformed payloads and unknown levels', () async {
    reply = (_) => <String, Object>{
      'securityLevel': 'quantum',
      'isHardwareBacked': true,
    };

    await expectLater(
      plugin.getPaymentKeyInfo(alias: 'k1'),
      throwsA(isA<NativeProtocolException>()),
    );
  });
}
