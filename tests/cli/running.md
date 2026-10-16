# Running fixture specifications

These commands only discover `tests/fixtures`, never `tests/cli`.
The fixtures invoke help, version, or invalid CLI options, so no suite loops back.

## `mm tests/fixtures/passing`

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
extra.md
PASS mm --version
pass 4
fail 0
```

## `mm tests/fixtures/passing --list`

```expected
basic.md
mm --help
mm --version
mm --not-an-option | mm --help
extra.md
mm --version
```

## `mm tests/fixtures/setup-failure --cwd tests/fixtures/nonexistent --list`

Listing must not execute the failing setup or require an existing working directory.

```expected
setup.md
mm --version
```

## `mm tests/fixtures/mismatch` (exit 1)

```expected-contains
pass 0
fail 1
FAIL mm --help: Line 1 mismatch:
expected: "not the help output"
actual: "Standalone Markdown-driven CLI end-to-end test runner"
```

## `mm tests/fixtures/invalid` (exit 1)

```expected
Error: Invalid spec invalid.md: Frontmatter tags must be an array of strings
```

## `mm tests/fixtures/setup-failure` (exit 1)

```expected-contains
pass 0
fail 1
FAIL setup: Setup command failed: mm --not-an-option
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```
