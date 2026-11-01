import 'package:flutter/material.dart';
import 'package:flutter_modular/flutter_modular.dart';

import 'core/core_module.dart';
import 'core/routes/app_routes.dart';
import 'features/lab/lab_module.dart';

final appModule = createModule(
  register: (context) {
    context
      ..module(coreModule)
      ..module(labModule)
      ..route(
        AppRoutes.root,
        guards: [(state) => AppRoutes.lab],
        child: (_, _) => const SizedBox.shrink(),
      );
  },
);
