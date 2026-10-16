# Match command output

Output matching ignores blank lines, repeated whitespace, and path separators.
The [test CLI](../support/README.md) prints known output: use `expected` for the full
result, or `expected-contains` for selected whole lines.

## `mm-test-cli lines "  one   two  " "" three`

Whitespace and blank lines are normalized before comparison.

```expected
one two
three
```

## `mm-test-cli echo 'path\to\file'`

Backslashes are literal command arguments, but match forward slashes in output.

```expected
path/to/file
```

## `mm-test-cli lines wrapped over "three lines"`

A line wrapped across two or three terminal lines can still match one expected line.

```expected
wrapped over three lines
```

## `mm-test-cli lines noise first noise second`

Contains matching skips other lines before and between these lines, not after them.

```expected-contains
first
second
```

## `mm-test-cli warn`

Successful commands compare stdout only. This helper also writes to stderr.

```expected
stdout
```

See [matching errors](matching-errors.md) for whole-line, ordering, and trailing-output failures.
