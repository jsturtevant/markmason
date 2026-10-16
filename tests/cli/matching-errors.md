# Output matching errors

These broken specs deliberately fail. Contains matching requires whole lines in
order, with no unmatched output after the last expected line.

## `mm tests/fixtures/matching-substring` (exit 1)

[`first` does not match `prefix first`](../fixtures/matching-substring/output.md).
Contains matching is not a substring search.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli lines "prefix first": Expected line 1 not found in output:
expected: "first"
```

## `mm tests/fixtures/matching-order` (exit 1)

[Both lines exist](../fixtures/matching-order/output.md), but in the wrong order.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli lines second first: Expected line 2 not found in output:
expected: "second"
```

## `mm tests/fixtures/matching-trailing` (exit 1)

[Extra output after the match](../fixtures/matching-trailing/output.md) still fails.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli lines first trailing: Actual output has 1 unmatched trailing line(s):
trailing
```
