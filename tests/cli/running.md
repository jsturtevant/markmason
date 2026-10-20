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

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*RUN  extra\.md.*✓ PASS  extra\.md  1/1 blocks.*Documents: 2 passed, 0 failed.*Blocks: 3 passed, 0 failed
```

With no failures, `mm` exits with code 0.

## Preview commands without running them

```markmason
mm tests/fixtures/passing --list
```

Preview commands without running them.

```expected-regex
(?s)basic\.md.*Block 1/2 — Show help.*mm --help.*Block 2/2 — Show the installed version.*mm --version.*extra\.md.*Block 1/1 — Show the installed version.*mm --version
```

No recursion: these fixtures only call help or version, never this guide's directory.

## Discover nested specs

```markmason
mm tests/fixtures/discovery --list
```

Discovery includes [nested Markdown files](../fixtures/discovery).
Paths in the listing are relative to the selected spec directory.

```expected-regex
(?s)a\.md.*Block 1/1 — Run a top-level command.*mm-test-cli echo top.*nested/b\.md.*Block 1/1 — Run a nested command.*mm-test-cli echo nested
```

## Run nested specs in order

```markmason
mm tests/fixtures/discovery
```

Both discovered files run, in path order.

```expected-regex
(?s)RUN  a\.md.*✓ PASS  a\.md  1/1 blocks.*RUN  nested/b\.md.*✓ PASS  nested/b\.md  1/1 blocks.*Documents: 2 passed, 0 failed.*Blocks: 2 passed, 0 failed
```

## Choose the working directory

```markmason
mm tests/fixtures/working-directory --cwd tests/fixtures/passing
```

`--cwd` controls where commands run, not where specs are found.
[This spec](../fixtures/working-directory/location.md) checks the actual directory.

```expected-regex
(?s)RUN  location\.md.*✓ PASS  location\.md  1/1 blocks.*Documents: 1 passed, 0 failed.*Blocks: 1 passed, 0 failed
```

## Show output and comparison details

```markmason
mm tests/fixtures/diagnostics --verbose
```

Show captured output and a comparison diff, even when the assertion passes.

```expected-regex
(?s)RUN  output\.md.*ACTUAL:.*hello.*DIFF \(expected vs actual\):.*✓ PASS Block 1/1 — Print a short result for diagnostics.*Blocks: 1 passed, 0 failed.*Duration:
```

[Filter tests](filtering.md) | [Error cases](errors.md)
