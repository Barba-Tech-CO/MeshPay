import 'package:path_provider/path_provider.dart';

import 'app_support_directory_service.dart';

class PathProviderAppSupportDirectoryService
    implements AppSupportDirectoryService {
  @override
  Future<String> getPath() async {
    return (await getApplicationSupportDirectory()).path;
  }
}
