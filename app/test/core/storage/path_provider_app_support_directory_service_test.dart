import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/storage/path_provider_app_support_directory_service.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const methodChannel = MethodChannel('plugins.flutter.io/path_provider');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(() {
    messenger.setMockMethodCallHandler(methodChannel, null);
  });

  test('returns the application support directory path', () async {
    messenger.setMockMethodCallHandler(methodChannel, (MethodCall call) async {
      expect(call.method, 'getApplicationSupportDirectory');
      return '/app/support';
    });

    final service = PathProviderAppSupportDirectoryService();

    expect(await service.getPath(), '/app/support');
  });
}
