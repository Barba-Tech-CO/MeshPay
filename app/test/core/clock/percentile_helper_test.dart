import 'package:flutter_test/flutter_test.dart';

import 'package:meshpay/core/clock/percentile_helper.dart';

void main() {
  test('calculates nearest-rank percentiles without mutating input', () {
    final values = <num>[9, 1, 5, 3, 7];

    expect(PercentileHelper.percentile(values, 0.5), 5);
    expect(PercentileHelper.percentile(values, 0.95), 9);
    expect(values, <num>[9, 1, 5, 3, 7]);
  });

  test('uses the minimum for the zero percentile', () {
    expect(PercentileHelper.percentile(<num>[7, 2, 4], 0), 2);
  });

  test('rejects empty samples and invalid percentiles', () {
    expect(
      () => PercentileHelper.percentile(<num>[], 0.5),
      throwsArgumentError,
    );
    expect(
      () => PercentileHelper.percentile(<num>[1], -0.1),
      throwsArgumentError,
    );
    expect(
      () => PercentileHelper.percentile(<num>[1], 1.1),
      throwsArgumentError,
    );
    expect(
      () => PercentileHelper.percentile(<num>[1], double.nan),
      throwsArgumentError,
    );
  });
}
