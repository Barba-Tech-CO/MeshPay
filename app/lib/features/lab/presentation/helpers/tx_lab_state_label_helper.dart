import '../../../../core/tx/tx_lab_state.dart';

class TxLabStateLabelHelper {
  TxLabStateLabelHelper._();

  static String labelFor(TxLabState state) => switch (state) {
    TxLabState.idle => 'Ocioso',
    TxLabState.proposed => 'Proposto',
    TxLabState.locked => 'Bloqueado',
    TxLabState.committed => 'Confirmado',
    TxLabState.failed => 'Falhou',
  };
}
