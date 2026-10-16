# Filter tests

These examples use [two sample specs](../fixtures/passing).
Run from the repository root with `mm` on `PATH`.

| File | Tags | Cases |
|------|------|-------|
| [basic.md](../fixtures/passing/basic.md) | `smoke`, `cli` | 3 |
| [extra.md](../fixtures/passing/extra.md) | `extra` | 1 |

## `mm tests/fixtures/passing --file-pattern basic`

`--file-pattern` matches file paths only. Here it runs all three cases in
`basic.md`, not individual command headings.

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --file-exclude basic`

Skip files whose paths match `basic`.

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## `mm tests/fixtures/passing --tags SMOKE,cli`

Require both tags. Matching ignores case, so `SMOKE` matches `smoke`.

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --tags-exclude smoke`

Skip files with the `smoke` tag.

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## `mm tests/fixtures/passing --test-name-pattern basic.md`

`--test-name-pattern` matches file paths or command headings. Here the file name
matches, so all three cases run, just as with `--file-pattern basic`.

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --test-name-pattern "^mm --version$"`

Here the pattern matches command headings instead: only `mm --version` runs in
each file. `--file-pattern` cannot select individual commands.

```expected
basic.md
PASS mm --version
extra.md
PASS mm --version
pass 2
fail 0
```

Combine the flags to narrow files first, then select commands within them.
Earlier cases won't run automatically; select the whole file if cases depend on
each other.

## `mm tests/fixtures/passing --tags missing` (exit 1)

No matches is an error, not an empty successful run.

```expected
Error: No markdown test specs matched in $REPO_ROOT/tests/fixtures/passing
```

`$REPO_ROOT` is replaced with the working directory.

See the [filter reference](../../docs/specification.md#filtering-and-diagnostics)
for environment variables.
