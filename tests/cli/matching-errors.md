# Output matching errors

These broken specs deliberately fail. Contains matching requires whole lines in
order, with no unmatched output after the last expected line.

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
