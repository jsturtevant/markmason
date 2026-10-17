# Run Markdown tests

Run a directory of specs, including nested files, or preview commands without executing them.
Choose the command's working directory and turn on diagnostics when needed.

Run these examples from the repository root with `mm` on `PATH`.
They use [sample specs](../fixtures/passing); bare `mm` defaults to `tests/cli`.

## Run a directory of specs

```markmason
mm tests/fixtures/passing
```

Run every `.md` file in the directory. `basic.md` has two cases;
`extra.md` has one.

```expected
basic.md
PASS mm --help
PASS mm --version
extra.md
PASS mm --version
pass 3
fail 0
```

With no failures, `mm` exits with code 0.

## Preview commands without running them

```markmason
mm tests/fixtures/passing --list
```

Preview commands without running them.

```expected
basic.md
mm --help
mm --version
extra.md
mm --version
```

No recursion: these fixtures only call help or version, never this guide's directory.

## Discover nested specs

```markmason
mm tests/fixtures/discovery --list
```

Discovery includes [nested Markdown files](../fixtures/discovery).
Paths in the listing are relative to the selected spec directory.

```expected
a.md
mm-test-cli echo top
nested/b.md
mm-test-cli echo nested
```

## Run nested specs in order

```markmason
mm tests/fixtures/discovery
```

Both discovered files run, in path order.

```expected
a.md
PASS mm-test-cli echo top
nested/b.md
PASS mm-test-cli echo nested
pass 2
fail 0
```

## Choose the working directory

```markmason
mm tests/fixtures/working-directory --cwd tests/fixtures/passing
```

`--cwd` controls where commands run, not where specs are found.
[This spec](../fixtures/working-directory/location.md) checks the actual directory.

```expected
location.md
PASS mm-test-cli cwd
pass 1
fail 0
```

## Show output and comparison details

```markmason
mm tests/fixtures/diagnostics --verbose
```

Show captured output and a comparison diff, even when the assertion passes.

```expected-contains
output.md
ACTUAL:
hello
DIFF (expected vs actual):
hello
PASS mm-test-cli echo hello
pass 1
fail 0
```

[Filter tests](filtering.md) | [Error cases](errors.md)
