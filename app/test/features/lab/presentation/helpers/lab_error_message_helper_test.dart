import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/features/lab/presentation/helpers/lab_error_message_helper.dart';

void main() {
  test('maps Bluetooth disabled to the actionable Portuguese message', () {
    final exception = PlatformException(
      code: 'bluetooth_disabled',
      message: 'Bluetooth is disabled.',
    );

    expect(
      LabErrorMessageHelper.messageFor(exception),
      'O Bluetooth está desligado. Ligue o Bluetooth (e desligue o modo avião) e tente de novo.',
    );
  });

  test('maps every native plugin error code to a Portuguese message', () {
    const expectedMessages = <String, String>{
      'advertising_already_started': 'O anúncio Bluetooth já está ativo.',
      'bluetooth_unavailable':
          'Bluetooth não está disponível neste dispositivo.',
      'advertising_failed': 'Não foi possível iniciar o anúncio Bluetooth.',
      'scan_already_started': 'A busca Bluetooth já está ativa.',
      'scan_failed': 'Não foi possível iniciar a busca Bluetooth.',
      'permission_denied': 'A permissão de Bluetooth foi negada. Verifique as permissões e tente novamente.',
      'invalid_scan_payload':
          'Os dados recebidos na busca Bluetooth são inválidos.',
      'invalid_session_id': 'O ID de sessão é inválido.',
      'l2cap_already_listening': 'O receptor L2CAP já está ativo.',
      'invalid_psm': 'O PSM L2CAP é inválido.',
      'invalid_channel': 'O canal L2CAP não é válido ou não está mais ativo.',
      'l2cap_io_error': 'Falha na comunicação L2CAP.',
      'channel_closed': 'O canal L2CAP foi fechado.',
      'l2cap_too_many_connections': 'O limite de conexões L2CAP foi atingido.',
      'invalid_device_id': 'O dispositivo Bluetooth selecionado é inválido.',
      'l2cap_connect_failed': 'Não foi possível conectar pelo canal L2CAP.',
      'bluetooth_io_error': 'Falha na comunicação Bluetooth.',
      'invalid_argument': 'Um parâmetro da operação é inválido.',
      'invalid_arguments': 'Os parâmetros da operação são inválidos.',
      'invalid_data': 'Os dados enviados são inválidos.',
      'invalid_echo_size': 'O tamanho ou a quantidade de ecos é inválido.',
      'echo_payload_mismatch':
          'O eco recebido não corresponde aos dados enviados.',
      'echo_interrupted': 'O teste de eco foi interrompido.',
      'echo_invalid_response':
          'O dispositivo retornou uma resposta de eco inválida.',
      'echo_failed': 'Não foi possível concluir o teste de eco.',
      'echo_handler_missing': 'O serviço de eco não está disponível.',
      'echo_timeout': 'O teste de eco excedeu o tempo limite.',
    };

    for (final entry in expectedMessages.entries) {
      expect(
        LabErrorMessageHelper.messageFor(
          PlatformException(code: entry.key, message: 'raw native message'),
        ),
        entry.value,
        reason: entry.key,
      );
    }
  });

  test('shows an unknown native code without exposing its message', () {
    expect(
      LabErrorMessageHelper.messageFor(
        PlatformException(code: 'future_code', message: 'raw native message'),
      ),
      'Falha no canal nativo: future_code',
    );
  });

  test('uses a generic message for non-platform errors', () {
    expect(
      LabErrorMessageHelper.messageFor(StateError('private error details')),
      'Ocorreu um erro inesperado. Tente novamente.',
    );
  });
}
