class PercentileHelper {
  PercentileHelper._();

  static double? percentileOrNull(List<num> values, double percentile) =>
      values.isEmpty
      ? null
      : PercentileHelper.percentile(values, percentile).toDouble();

  static num percentile(List<num> values, double percentile) {
    if (values.isEmpty ||
        !percentile.isFinite ||
        percentile < 0 ||
        percentile > 1) {
      throw ArgumentError(
        'A percentile requires a non-empty sample and a value from 0 to 1.',
      );
    }

    final sortedValues = List<num>.of(values)..sort();
    final rank = (percentile * sortedValues.length).ceil().clamp(
      1,
      sortedValues.length,
    );
    return sortedValues[rank - 1];
  }
}
