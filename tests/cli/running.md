# Run Markdown tests

Run these examples from the repository root with `mm` on `PATH`.
They use [sample specs](../fixtures); bare `mm` defaults to `tests/cli`.

## `mm tests/fixtures/passing`

Run every `.md` file in the directory. `basic.md` has three cases;
`extra.md` has one.

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

A `PASS` means output and exit code matched, including expected errors.

## `mm tests/fixtures/passing --list`

Preview commands without running them.

```expected
basic.md
mm --help
mm --version
mm --not-an-option | mm --help
extra.md
mm --version
```

## `mm tests/fixtures/setup-failure --cwd tests/fixtures/nonexistent --list`

Even broken setup and a nonexistent `--cwd` don't affect listing.

```expected
setup.md
mm --version
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

No recursion: these fixtures only call help, version, or invalid options.

[Filter tests](filtering.md)
