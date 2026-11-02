import '../../../../core/noise/noise_handshake_status.dart';

class NoiseHandshakeLabelHelper {
  NoiseHandshakeLabelHelper._();

  static String labelFor(NoiseHandshakeStatus status) => switch (status) {
    NoiseHandshakeStatus.idle => 'Aguardando conexão',
    NoiseHandshakeStatus.inProgress => 'Em andamento',
    NoiseHandshakeStatus.finished => 'Concluído',
    NoiseHandshakeStatus.failed => 'Falhou',
  };
}
