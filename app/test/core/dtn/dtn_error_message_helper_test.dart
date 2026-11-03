import 'package:flutter_test/flutter_test.dart';
import 'package:meshpay/core/dtn/dtn_error_message_helper.dart';

void main() {
  // Exact Debug names of the rejecting variants of Rust `IngestOutcome`
  // (core/src/dtn_ingest_outcome.rs); `Stored` and `Duplicate` are not errors.
  const outcomes = <String>[
    'Refused',
    'RateLimited',
    'SessionBudgetExceeded',
    'Oversize',
    'Malformed',
    'IdMismatch',
    'HopsExceeded',
    'Expired',
    'FromFuture',
    'UnverifiedProofs',
  ];
  const generic = 'O buffer recusou o envelope.';

  test('maps each of the 10 real engine strings to its own message', () {
    final messages = <String>{};
    for (final outcome in outcomes) {
      final name = DtnErrorMessageHelper.outcomeOf(
        'envelope rejected: $outcome',
      );

      expect(name, outcome);
      final message = DtnErrorMessageHelper.messageFor(name!);
      expect(message, isNot(generic));
      messages.add(message);
    }
    expect(messages, hasLength(outcomes.length));
  });

  test('other engine errors are not rejections', () {
    expect(
      DtnErrorMessageHelper.outcomeOf(
        'snapshot version not newer than the stored one',
      ),
      isNull,
    );
    expect(DtnErrorMessageHelper.outcomeOf('database is locked'), isNull);
  });

  test('finds the outcome when the engine string is wrapped', () {
    expect(
      DtnErrorMessageHelper.outcomeOf(
        'AnyhowException(envelope rejected: Oversize)',
      ),
      'Oversize',
    );
    expect(
      DtnErrorMessageHelper.outcomeOf('wrapped: envelope rejected: Expired'),
      'Expired',
    );
    expect(
      DtnErrorMessageHelper.outcomeOf('envelope rejected: Malformed\nat frame'),
      'Malformed',
    );
  });

  test('a missing prefix or a prefix with no outcome is not a rejection', () {
    expect(
      DtnErrorMessageHelper.outcomeOf('envelope rejected: '),
      isNull,
    );
    expect(
      DtnErrorMessageHelper.outcomeOf('envelope rejected: 42'),
      isNull,
    );
    expect(
      DtnErrorMessageHelper.outcomeOf('AnyhowException(database is locked)'),
      isNull,
    );
  });

  test('an unknown or partial outcome name gets the generic message', () {
    expect(DtnErrorMessageHelper.messageFor('Surprise'), generic);
    expect(DtnErrorMessageHelper.messageFor('Expired!'), generic);
    expect(DtnErrorMessageHelper.messageFor('SessionBudget'), generic);
  });
}
