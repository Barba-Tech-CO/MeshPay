import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/clock/clock_log_entry.dart';
import 'package:meshpay/core/clock/json_lines_clock_log_repository.dart';
import 'package:meshpay_native/clock_sample.dart';

import '../../fakes/fake_app_support_directory_service.dart';

void main() {
  late Directory directory;
  late JsonLinesClockLogRepository repository;

  setUp(() async {
    directory = await Directory.systemTemp.createTemp('meshpay-clock-log-');
    repository = JsonLinesClockLogRepository(
      FakeAppSupportDirectoryService(directory.path),
    );
  });

  tearDown(() async {
    await directory.delete(recursive: true);
  });

  test('appends typed JSON-lines entries with increasing sequences', () async {
    final launch = await repository.append(
      sample: const ClockSample(elapsedRealtimeNanos: 100, bootCount: 4),
      label: 'Início do app',
      isAppLaunch: true,
    );
    final sample = await repository.append(
      sample: const ClockSample(elapsedRealtimeNanos: 220, bootCount: 4),
      label: 'Amostra manual',
      isAppLaunch: false,
    );

    expect(launch.sequence, 1);
    expect(launch.isAppLaunch, isTrue);
    expect(sample.sequence, 2);
    expect(await repository.readAll(), <ClockLogEntry>[launch, sample]);
    expect(File('${directory.path}/clock_log.jsonl').existsSync(), isTrue);
  });

  test('continues sequence after constructing a new repository', () async {
    await repository.append(
      sample: const ClockSample(elapsedRealtimeNanos: 10, bootCount: null),
      label: 'Antes',
      isAppLaunch: false,
    );
    final restartedRepository = JsonLinesClockLogRepository(
      FakeAppSupportDirectoryService(directory.path),
    );

    final next = await restartedRepository.append(
      sample: const ClockSample(elapsedRealtimeNanos: 5, bootCount: 5),
      label: 'Depois do reinício',
      isAppLaunch: false,
    );

    expect(next.sequence, 2);
    expect(next.elapsedRealtimeNanos, 5);
    expect(next.bootCount, 5);
  });
}
