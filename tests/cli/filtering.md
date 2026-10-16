# Filter tests

These examples use [two sample specs](../fixtures/passing).
Run from the repository root with `mm` on `PATH`.

| File | Tags | Cases |
|------|------|-------|
| [basic.md](../fixtures/passing/basic.md) | `smoke`, `cli` | 3 |
| [extra.md](../fixtures/passing/extra.md) | `extra` | 1 |

## `mm tests/fixtures/passing --file-pattern basic`

Run files whose relative paths match the regex `basic`.

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

Name filters match file paths too. A file match runs all its cases.

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --test-name-pattern "^mm --version$"`

Match the exact command heading in either file. The anchors prevent partial matches.

```expected
basic.md
PASS mm --version
extra.md
PASS mm --version
pass 2
fail 0
```

Earlier cases won't run automatically. Select the whole file if cases depend on
each other.

## `mm tests/fixtures/passing --tags missing` (exit 1)

No matches is an error, not an empty successful run.

```expected
Error: No markdown test specs matched in $REPO_ROOT/tests/fixtures/passing
```

`$REPO_ROOT` is replaced with the working directory.

See the [filter reference](../../docs/specification.md#filtering-and-diagnostics)
for environment variables.
