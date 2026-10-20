# Output matching errors

These broken specs deliberately fail. Contains matching requires whole lines in
order, with no unmatched output after the last expected line. Substring and regex
assertions instead fail when their text or pattern cannot be found.

## Reject partial-line matches

```markmason (exit 1)
mm tests/fixtures/matching-substring
```

[`first` does not match `prefix first`](../fixtures/matching-substring/output.md).
Contains matching is not a substring search.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli lines "prefix first": Expected line 1 not found in output:
expected: "first"
```

## Reject lines in the wrong order

```markmason (exit 1)
mm tests/fixtures/matching-order
```

[Both lines exist](../fixtures/matching-order/output.md), but in the wrong order.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli lines second first: Expected line 2 not found in output:
expected: "second"
```

## Reject unmatched trailing output

```markmason (exit 1)
mm tests/fixtures/matching-trailing
```

[Extra output after the match](../fixtures/matching-trailing/output.md) still fails.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli lines first trailing: Actual output has 1 unmatched trailing line(s):
trailing
```

## Report a missing substring

The [missing-phrase spec](../fixtures/substring-mismatch/output.md) reports the
literal text it wanted alongside the full output.

```markmason (exit 1)
mm tests/fixtures/substring-mismatch
```

```expected-substring
Expected substring not found in output:
  expected: "missing phrase"
Actual output:
prefix first
second trailing
```

## Report a missing regex match

The [numeric-ID spec](../fixtures/regex-mismatch/output.md) receives a word
instead. Diagnostics quote the pattern, so a literal backslash displays as `\\`.

```markmason (exit 1)
mm tests/fixtures/regex-mismatch
```

```expected-substring
Expected regex not found in output:
  expected: "id: \\d+"
Actual output:
id: unknown
request complete
```

## Reject malformed patterns before execution

The [invalid-regex spec](../fixtures/invalid-regex/output.md) has an unclosed
character class. Its command never runs.

```markmason (exit 1)
mm tests/fixtures/invalid-regex
```

```expected-substring
Invalid expected-regex pattern: regex parse error:
    [
    ^
error: unclosed character class
```

## Match stderr from an expected failure

For a failing command, matching includes stdout followed by stderr. Declaring the
exit code is still required; finding the phrase alone does not excuse a failure.

```markmason (exit 2)
mm-test-cli fail
```

```expected-regex
(?s)stdout.*stderr
```
