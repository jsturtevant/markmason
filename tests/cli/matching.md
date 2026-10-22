# Match command output

Use `expected` for the full result, or `expected-contains` for selected whole lines.
Both normalize whitespace and paths. Use `expected-substring` or `expected-regex`
to match part of the [test CLI](../support/README.md)'s output without rewriting it.

## Normalize whitespace and blank lines

````markdown markmason
```markmason
mm-test-cli lines "  one   two  " "" three
```

Whitespace and blank lines are normalized before comparison.

```expected
one two
three
```
````

## Match paths across platforms

````markdown markmason
```markmason
mm-test-cli echo 'path\to\file'
```

Backslashes are literal command arguments, but match forward slashes in output.

```expected
path/to/file
```
````

## Match wrapped terminal lines

````markdown markmason
```markmason
mm-test-cli lines wrapped over "three lines"
```

A line wrapped across two or three terminal lines can still match one expected line.

```expected
wrapped over three lines
```
````

## Match selected lines in order

````markdown markmason
```markmason
mm-test-cli lines noise first noise second
```

Contains matching skips other lines before and between these lines, not after them.

```expected-contains
first
second
```
````

## Ignore stderr for successful commands

````markdown markmason
```markmason
mm-test-cli warn
```

Successful commands compare stdout only. This helper also writes to stderr.

```expected
stdout
```
````

## Match a stable phrase inside a longer line

Keep the full error text visible without asserting the generated user or request ID.
Substring matching allows extra output before and after the phrase.

````markdown markmason
```markmason
mm-test-cli lines 'Error from server: user "demo-482" is not authorized to xyz' 'request ID: abc-123'
```

```expected-substring
is not authorized to xyz
```
````

## Keep punctuation literal

Substring matching treats backslashes, brackets, and parentheses as ordinary text.
It does not normalize paths or repeated spaces.

````markdown markmason
```markmason
mm-test-cli echo 'Saved C:\work\report[1].txt (ready)'
```

```expected-substring
C:\work\report[1].txt (ready)
```
````

## Match a generated name and duration

Regex matching searches the output using Rust regex syntax. Write `\d`, not `\\d`;
Markdown fences do not need another escaping layer.

````markdown markmason
```markmason
mm-test-cli lines 'Created pod demo-482 in 1.25s' 'next step: inspect logs'
```

```expected-regex
Created pod demo-\d+ in \d+\.\d+s
```
````

## Match across line boundaries

A substring block is one literal piece of text, including its internal newlines.
The fence's final newline is not part of the assertion.

````markdown markmason
```markmason
mm-test-cli lines 'prefix first' 'second trailing'
```

```expected-substring
first
second
```
````

## Use regex flags and escape punctuation

Use `(?m)` for line anchors or `(?s)` to let `.` span newlines.
Escape literal regex punctuation and backslashes.

````markdown markmason
```markmason
mm-test-cli lines 'Saved C:\work\report[1].txt (ready)' 'next step: inspect logs'
```

```expected-regex
(?m)^Saved C:\\work\\report\[1\]\.txt \(ready\)$
```
````

See [matching errors](matching-errors.md) for mismatches and invalid patterns.
