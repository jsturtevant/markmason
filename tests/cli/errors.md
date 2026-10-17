# Error cases

These examples deliberately use bad arguments or broken specs.
For normal usage, start with [running tests](running.md) or [filtering](filtering.md).

`(exit N)` sets the expected exit code. An expected error counts as a passing test.

## Reject unknown options

```markmason (exit 2)
mm --not-an-option
```

Unknown options exit with code 2. No tests run.

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## Reject a zero timeout

```markmason (exit 2)
mm --timeout 0
```

Timeouts are in seconds and must be greater than zero.

```expected-contains
error: invalid value '0' for '--timeout <TIMEOUT>': timeout must be positive, finite seconds
For more information, try '--help'.
```

## Stop a pipeline on failure

```markmason (exit 2)
mm --not-an-option | mm --help
```

A pipeline stops at the first failing command. The second command must not run.

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## Report an output mismatch

```markmason (exit 1)
mm tests/fixtures/mismatch
```

[This spec](../fixtures/mismatch/mismatch.md) expects the wrong text.
The command succeeds, but the assertion fails and `mm` exits with code 1.

```expected-contains
pass 0
fail 1
FAIL mm --help: Line 1 mismatch:
expected: "not the help output"
actual: "Standalone Markdown-driven CLI end-to-end test runner"
```

## Reject invalid frontmatter

```markmason (exit 1)
mm tests/fixtures/invalid
```

[This spec](../fixtures/invalid/invalid.md) uses a string for `tags`.
Tags must be a YAML array, such as `[smoke, cli]`.

```expected
Error: Invalid spec invalid.md: Frontmatter tags must be an array of strings
```

## Skip cases after setup fails

```markmason (exit 1)
mm tests/fixtures/setup-failure
```

Setup runs before a file's cases. If it fails, those cases are skipped.

```expected-contains
pass 0
fail 1
FAIL setup: Setup command failed: mm --not-an-option
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## List cases without running setup

```markmason
mm tests/fixtures/setup-failure --cwd tests/fixtures/nonexistent --list
```

Listing still works with broken setup and a nonexistent `--cwd`: neither is used.

```expected
setup.md
mm --version
```

## Report filters with no matches

```markmason (exit 1)
mm tests/fixtures/passing --tags missing
```

No matches is an error, not an empty successful run.

```expected
Error: No markdown test specs matched in $REPO_ROOT/tests/fixtures/passing
```

`$REPO_ROOT` is replaced with the working directory.

## Compare stdout and stderr for failures

```markmason (exit 2)
mm-test-cli fail
```

For a nonzero exit, compare stdout followed by stderr.
Successful commands compare [stdout only](matching.md).

```expected
stdout
stderr
```

## Stop a command block on failure

The first nonzero exit stops the block. Output from earlier commands is kept,
including stdout and stderr from the failed command.

```markmason (exit 2)
mm-test-cli echo before
mm-test-cli fail
mm-test-cli echo must-not-run
```

```expected
before
stdout
stderr
```

See [matching errors](matching-errors.md) and [runtime errors](runtime-errors.md)
for failed assertions, timeouts, and incomplete terminal scripts.
