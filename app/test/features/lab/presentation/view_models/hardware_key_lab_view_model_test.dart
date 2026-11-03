import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/view_models/hardware_key_lab_view_model.dart';
import 'package:meshpay_native/key_security_level.dart';
import 'package:meshpay_native/payment_key_exception.dart';

import '../../../../fakes/fake_hardware_key_lab_service.dart';

void main() {
  late FakeHardwareKeyLabService service;
  late HardwareKeyLabViewModel viewModel;

  setUp(() {
    service = FakeHardwareKeyLabService();
    viewModel = HardwareKeyLabViewModel(service);
  });

  test('generates, signs, reports and deletes the probe key', () async {
    await viewModel.runProbe();

    expect(service.calls, <String>[
      'capabilities',
      'delete:lab_probe',
      'generate:lab_probe',
      'sign:lab_probe',
      'delete:lab_probe',
    ]);
    final result = viewModel.result!;
    expect(result.securityLevel, KeySecurityLevel.tee);
    expect(result.chainLength, 3);
    expect(result.apiLevel, 34);
    expect(result.signatureLength, 71);
    expect(result.publicKey, hasLength(65));
    expect(viewModel.error, isNull);
    expect(viewModel.isRunning, isFalse);
  });

  test('requiring StrongBox on a device without it shows the error', () async {
    viewModel.setRequireStrongBox(true);

    await viewModel.runProbe();

    expect(service.lastRequireStrongBox, isTrue);
    expect(viewModel.result, isNull);
    expect(viewModel.error, contains('StrongBox'));
    expect(service.calls, <String>[
      'capabilities',
      'delete:lab_probe',
      'generate:lab_probe',
    ]);
  });

  test('requiring StrongBox where it exists reports strongBox', () async {
    service.hasStrongBox = true;
    viewModel.setRequireStrongBox(true);

    await viewModel.runProbe();

    expect(viewModel.result!.securityLevel, KeySecurityLevel.strongBox);
    expect(viewModel.result!.hasStrongBox, isTrue);
  });

  test(
    'maps alias_exists and does not delete after a failed generate',
    () async {
      service.generateError = const PaymentKeyException('alias_exists', null);

      await viewModel.runProbe();

      expect(viewModel.error, contains('Já existe uma chave'));
      expect(
        service.calls.where((call) => call.startsWith('delete')),
        hasLength(1),
      );
    },
  );

  test('removes a stale probe key first and ignores key_not_found', () async {
    service.deleteErrors.add(const PaymentKeyException('key_not_found', null));

    await viewModel.runProbe();

    expect(viewModel.error, isNull);
    expect(viewModel.result, isNotNull);
  });

  test('a failing pre-delete aborts before generating', () async {
    service.deleteErrors.add(const PaymentKeyException('keystore_error', null));

    await viewModel.runProbe();

    expect(viewModel.error, contains('Keystore'));
    expect(service.calls, isNot(contains('generate:lab_probe')));
  });

  test('a delete failure is reported after a successful probe', () async {
    service.deleteErrors.addAll(<Object?>[
      null,
      const PaymentKeyException('keystore_error', null),
    ]);

    await viewModel.runProbe();

    expect(viewModel.result, isNotNull);
    expect(viewModel.error, contains('Keystore'));
  });
}
