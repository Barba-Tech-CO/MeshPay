import 'dart:typed_data';

import 'native_protocol_exception.dart';

class NativePayload {
  static Map<Object?, Object?> readMap(Object? value, String name) {
    if (value is Map<Object?, Object?>) {
      return value;
    }
    throw NativeProtocolException('$name must be a map.');
  }

  static String readString(Map<Object?, Object?> payload, String key) {
    final Object? value = payload[key];
    if (value is String) {
      return value;
    }
    throw NativeProtocolException('$key must be a string.');
  }

  static int readInt(Object? value, String name) {
    if (value is int) {
      return value;
    }
    throw NativeProtocolException('$name must be an integer.');
  }

  static int readIntField(Map<Object?, Object?> payload, String key) {
    return readInt(payload[key], key);
  }

  static int? readOptionalInt(Map<Object?, Object?> payload, String key) {
    final Object? value = payload[key];
    if (value == null) {
      return null;
    }
    if (value is int) {
      return value;
    }
    throw NativeProtocolException('$key must be an integer or null.');
  }

  static bool readBool(Map<Object?, Object?> payload, String key) {
    final Object? value = payload[key];
    if (value is bool) {
      return value;
    }
    throw NativeProtocolException('$key must be a boolean.');
  }

  static Uint8List readBytes(Object? value, String name) {
    if (value is Uint8List) {
      return Uint8List.fromList(value);
    }
    throw NativeProtocolException('$name must be a byte array.');
  }

  static List<int> readIntList(Object? value, String name) {
    if (value is! List<Object?>) {
      throw NativeProtocolException('$name must be a list of integers.');
    }
    final result = <int>[];
    for (final item in value) {
      if (item is! int) {
        throw NativeProtocolException('$name must contain only integers.');
      }
      result.add(item);
    }
    return List<int>.unmodifiable(result);
  }

  static Uint8List readBytesField(Map<Object?, Object?> payload, String key) {
    return readBytes(payload[key], key);
  }

  static List<Uint8List> readBytesList(Object? value, String name) {
    if (value is! List<Object?>) {
      throw NativeProtocolException('$name must be a list of byte arrays.');
    }
    return List<Uint8List>.unmodifiable(
      value.map((Object? item) => readBytes(item, name)),
    );
  }

  static List<String> readStringList(Object? value, String name) {
    if (value is! List<Object?>) {
      throw NativeProtocolException('$name must be a list of strings.');
    }
    final result = <String>[];
    for (final item in value) {
      if (item is! String) {
        throw NativeProtocolException('$name must contain only strings.');
      }
      result.add(item);
    }
    return List<String>.unmodifiable(result);
  }
}
