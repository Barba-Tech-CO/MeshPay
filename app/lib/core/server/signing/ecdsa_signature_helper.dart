import 'dart:typed_data';

/// The Keystore returns DER ECDSA signatures; the server parses raw 64-byte
/// r||s (`Signature::from_slice`). Converts and normalizes to low-S.
class EcdsaSignatureHelper {
  EcdsaSignatureHelper._();

  static const int _scalarBytes = 32;
  static const int _sequenceTag = 0x30;
  static const int _integerTag = 0x02;

  /// P-256 group order.
  static final BigInt _order = BigInt.parse(
    'ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551',
    radix: 16,
  );

  static Uint8List derToRawLowS(Uint8List der) {
    // Short-form lengths only: a P-256 DER signature is at most 72 bytes.
    if (der.length < 8 || der[0] != _sequenceTag || der[1] != der.length - 2) {
      throw const FormatException('Malformed DER signature.');
    }
    final (r, afterR) = _readInteger(der, 2);
    final (s, afterS) = _readInteger(der, afterR);
    if (afterS != der.length || r == BigInt.zero || s == BigInt.zero) {
      throw const FormatException('Malformed DER signature.');
    }
    final lowS = s > _order >> 1 ? _order - s : s;
    return Uint8List.fromList(<int>[..._toFixed(r), ..._toFixed(lowS)]);
  }

  static (BigInt, int) _readInteger(Uint8List der, int offset) {
    if (offset + 2 > der.length || der[offset] != _integerTag) {
      throw const FormatException('Malformed DER signature.');
    }
    final length = der[offset + 1];
    final start = offset + 2;
    final end = start + length;
    if (length == 0 || length > _scalarBytes + 1 || end > der.length) {
      throw const FormatException('Malformed DER signature.');
    }
    var value = BigInt.zero;
    for (var index = start; index < end; index++) {
      value = (value << 8) | BigInt.from(der[index]);
    }
    if (value >= _order) {
      throw const FormatException('Malformed DER signature.');
    }
    return (value, end);
  }

  static List<int> _toFixed(BigInt value) => <int>[
    for (var index = _scalarBytes - 1; index >= 0; index--)
      ((value >> (index * 8)) & BigInt.from(0xff)).toInt(),
  ];
}
