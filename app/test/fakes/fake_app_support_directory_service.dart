import 'package:meshpay/core/storage/app_support_directory_service.dart';

class FakeAppSupportDirectoryService implements AppSupportDirectoryService {
  FakeAppSupportDirectoryService(this.path);

  final String path;

  @override
  Future<String> getPath() async => path;
}
