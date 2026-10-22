# Run Markdown tests

Run a directory of specs, including nested files, or preview commands without executing them.
Choose the command's working directory and turn on diagnostics when needed.

Run these examples from the repository root with `mm` on `PATH`.
They use [sample specs](../fixtures/passing); bare `mm` defaults to `tests/cli`.

## Run a directory of specs

````markdown markmason
```markmason
mm tests/fixtures/passing
```

Run every `.md` file in the directory. `basic.md` has two cases;
`extra.md` has one.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
RUN  extra.md
  RUN  Block 1/1 — Show the installed version
  ✓ PASS Block 1/1 — Show the installed version
Blocks: 3 passed, 0 failed (includes deferred)
```
````

With no failures, `mm` exits with code 0.

## Preview commands without running them

````markdown markmason
```markmason
mm tests/fixtures/passing --list
```

Preview commands without running them.

```expected
basic.md
  Block 1/2 — Show help
    mm --help
  Block 2/2 — Show the installed version
    mm --version
extra.md
  Block 1/1 — Show the installed version
    mm --version
```
````

No recursion: these fixtures only call help or version, never this guide's directory.

## Discover nested specs

````markdown markmason
```markmason
mm tests/fixtures/discovery --list
```

Discovery includes [nested Markdown files](../fixtures/discovery).
Paths in the listing are relative to the selected spec directory.

```expected
a.md
  Block 1/1 — Run a top-level command
    mm-test-cli echo top
nested/b.md
  Block 1/1 — Run a nested command
    mm-test-cli echo nested
```
````

## Run nested specs in order

````markdown markmason
```markmason
mm tests/fixtures/discovery
```

Both discovered files run, in path order.

```expected-contains
RUN  a.md
  RUN  Block 1/1 — Run a top-level command
  ✓ PASS Block 1/1 — Run a top-level command
RUN  nested/b.md
  RUN  Block 1/1 — Run a nested command
  ✓ PASS Block 1/1 — Run a nested command
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Choose the working directory

````markdown markmason
```markmason
mm tests/fixtures/working-directory --cwd tests/fixtures/passing
```

`--cwd` controls where commands run, not where specs are found.
[This spec](../fixtures/working-directory/location.md) checks the actual directory.

```expected-contains
RUN  location.md
  RUN  Block 1/1 — Report the actual working directory
  ✓ PASS Block 1/1 — Report the actual working directory
Blocks: 1 passed, 0 failed (includes deferred)
```
````

## Show output and comparison details

````markdown markmason
```markmason
mm tests/fixtures/diagnostics --verbose
```

Show captured output and a comparison diff, even when the assertion passes.

```expected-contains
RUN  output.md
ACTUAL:
hello
DIFF (expected vs actual):
hello
✓ PASS Block 1/1 — Print a short result for diagnostics
Documents: 1 passed, 0 failed
Blocks: 1 passed, 0 failed (includes deferred)
```
````

## Stop after the first failed document

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/fail-fast --fail-fast
```

Fail-fast stops at a document boundary. The failed document's deferred cleanup
still runs, and the summary lists later selected documents as not run.

```expected-contains
RUN  a-fails.md
  ✗ FAIL  Block 1/2 — Produce the wrong value
  RUN  Always 2/2 — Finish cleanup
  ✓ PASS Always 2/2 — Finish cleanup
Documents: 0 passed, 1 failed, 1 not run
NOT RUN
- b-not-run.md 0/1 blocks
Blocks: 1 passed, 1 failed (includes deferred)
```
````

## Run independent files concurrently

````markdown markmason
```markmason
mm tests/fixtures/parallel --parallel 2
```

Parallelism is bounded by the requested worker count. Output is buffered and
printed in file order. A file with `parallel: false` waits for active files,
runs alone, and forms a barrier before later files start.

```expected-contains
RUN  a.md
  ✓ PASS Block 1/1 — Wait in the first worker
RUN  b.md
  ✓ PASS Block 1/1 — Wait in the second worker
RUN  c-serial.md
  ✓ PASS Block 1/1 — Run after the parallel files
Documents: 3 passed, 0 failed
Blocks: 3 passed, 0 failed (includes deferred)
```
````

[Filter tests](filtering.md) | [Error cases](errors.md)
