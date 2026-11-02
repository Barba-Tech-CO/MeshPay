import 'dart:math';
import 'dart:typed_data';

class RandomBytesHelper {
  RandomBytesHelper._();

  static Uint8List secure(int length) {
    final random = Random.secure();
    return Uint8List.fromList(<int>[
      for (var index = 0; index < length; index++) random.nextInt(256),
    ]);
  }
}
