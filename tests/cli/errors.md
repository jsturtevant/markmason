# Error cases

These examples deliberately use bad arguments or broken specs.
For normal usage, start with [running tests](running.md) or [filtering](filtering.md).

`(exit N)` sets the expected exit code. An expected error counts as a passing test.

## `mm --not-an-option` (exit 2)

Unknown options exit with code 2. No tests run.

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## `mm --timeout 0` (exit 2)

Timeouts are in seconds and must be greater than zero.

```expected-contains
error: invalid value '0' for '--timeout <TIMEOUT>': timeout must be positive, finite seconds
For more information, try '--help'.
```

## `mm --not-an-option | mm --help` (exit 2)

A pipeline stops at the first failing command. The second command must not run.

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## `mm tests/fixtures/mismatch` (exit 1)

[This spec](../fixtures/mismatch/mismatch.md) expects the wrong text.
The command succeeds, but the assertion fails and `mm` exits with code 1.

```expected-contains
pass 0
fail 1
FAIL mm --help: Line 1 mismatch:
expected: "not the help output"
actual: "Standalone Markdown-driven CLI end-to-end test runner"
```

## `mm tests/fixtures/invalid` (exit 1)

[This spec](../fixtures/invalid/invalid.md) uses a string for `tags`.
Tags must be a YAML array, such as `[smoke, cli]`.

```expected
Error: Invalid spec invalid.md: Frontmatter tags must be an array of strings
```

## `mm tests/fixtures/setup-failure` (exit 1)

Setup runs before a file's cases. If it fails, those cases are skipped.

```expected-contains
pass 0
fail 1
FAIL setup: Setup command failed: mm --not-an-option
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## `mm tests/fixtures/setup-failure --cwd tests/fixtures/nonexistent --list`

Listing still works with broken setup and a nonexistent `--cwd`: neither is used.

```expected
setup.md
mm --version
```

## `mm tests/fixtures/passing --tags missing` (exit 1)

No matches is an error, not an empty successful run.

```expected
Error: No markdown test specs matched in $REPO_ROOT/tests/fixtures/passing
```

`$REPO_ROOT` is replaced with the working directory.
