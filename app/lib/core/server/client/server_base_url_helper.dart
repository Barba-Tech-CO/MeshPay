/// Signed material and attestation chains only go to https servers, or to a
/// loopback host over plain http (lab server through `adb reverse`).
class ServerBaseUrlHelper {
  ServerBaseUrlHelper._();

  static const Set<String> _loopbackHosts = <String>{'127.0.0.1', 'localhost'};

  static bool isAllowed(String url) {
    final uri = Uri.tryParse(url);
    if (uri == null || uri.host.isEmpty) {
      return false;
    }
    return uri.scheme == 'https' ||
        (uri.scheme == 'http' && _loopbackHosts.contains(uri.host));
  }
}
