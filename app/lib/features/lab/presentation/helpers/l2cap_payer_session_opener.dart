import '../../../../core/ble/native_lab_service.dart';
import '../../../../core/noise/hex_helper.dart';
import '../../../../core/noise/noise_l2cap_session.dart';
import '../../../../core/noise/noise_prologue_helper.dart';
import '../../../../core/noise/noise_service.dart';
import 'l2cap_payer_input_helper.dart';

class L2capPayerSessionOpener {
  L2capPayerSessionOpener._();

  /// Builds the initiator Noise state, runs [beforeOpen] (stops the scan) and
  /// opens the L2CAP channel; the handshake is not run here.
  static Future<NoiseL2capSession> open({
    required NativeLabService nativeService,
    required NoiseService noiseService,
    required L2capPayerInput input,
    required String deviceId,
    required Future<void> Function() beforeOpen,
  }) async {
    final noise = noiseService.newInitiator(
      responderPublicKey: HexHelper.decode(input.keyHex),
      prologue: NoisePrologueHelper.build(
        sessionIdHex: input.sessionIdHex,
        psm: input.psm,
        keyHex: input.keyHex,
      ),
    );
    await beforeOpen();
    final channelId = await nativeService.openL2cap(
      deviceId: deviceId,
      psm: input.psm,
    );
    return NoiseL2capSession(
      nativeService: nativeService,
      channelId: channelId,
      noise: noise,
    );
  }

  /// Runs the initiator handshake; returns its duration in milliseconds.
  static Future<double> timeHandshake(NoiseL2capSession session) async {
    final stopwatch = Stopwatch()..start();
    await session.runInitiatorHandshake();
    return stopwatch.elapsedMicroseconds / 1000;
  }
}
