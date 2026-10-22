# Set filters in the environment

Environment variables select the same [sample specs](filtering.md) as CLI flags.
[`mm-test-cli with-env`](../support/README.md) sets them for one command, without shell-specific syntax.

## Include paths from the environment

````markdown markmason
```markmason
mm-test-cli with-env E2E_FILE_PATTERN basic -- mm tests/fixtures/passing
```

Include files whose relative paths match the pattern.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Exclude paths from the environment

````markdown markmason
```markmason
mm-test-cli with-env E2E_FILE_EXCLUDE basic -- mm tests/fixtures/passing
```

Exclude matching file paths.

```expected-contains
RUN  extra.md
  RUN  Block 1/1 — Show the installed version
  ✓ PASS Block 1/1 — Show the installed version
Blocks: 1 passed, 0 failed (includes deferred)
```
````

## Require tags from the environment

````markdown markmason
```markmason
mm-test-cli with-env E2E_TAGS SMOKE,cli -- mm tests/fixtures/passing
```

Require every listed tag, ignoring case.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Exclude tags from the environment

````markdown markmason
```markmason
mm-test-cli with-env E2E_TAGS_EXCLUDE smoke -- mm tests/fixtures/passing
```

Exclude files with any listed tag.

```expected-contains
RUN  extra.md
  RUN  Block 1/1 — Show the installed version
  ✓ PASS Block 1/1 — Show the installed version
Blocks: 1 passed, 0 failed (includes deferred)
```
````

## Choose a different variable prefix

````markdown markmason
```markmason
mm-test-cli with-env E2E_TAGS extra DOC_TAGS smoke -- mm tests/fixtures/passing --env-prefix DOC_
```

Change the filter prefix. `DOC_TAGS` is used; the default `E2E_TAGS` is ignored.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Override the include pattern

````markdown markmason
```markmason
mm-test-cli with-env E2E_FILE_PATTERN extra -- mm tests/fixtures/passing --file-pattern basic
```

An explicit file pattern replaces the environment's include pattern.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Override the exclude pattern

````markdown markmason
```markmason
mm-test-cli with-env E2E_FILE_EXCLUDE basic -- mm tests/fixtures/passing --file-exclude extra
```

An explicit file exclusion replaces the environment's exclusion.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Override required tags

````markdown markmason
```markmason
mm-test-cli with-env E2E_TAGS extra -- mm tests/fixtures/passing --tags smoke
```

Explicit tags replace the environment's required tags.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Override excluded tags

````markdown markmason
```markmason
mm-test-cli with-env E2E_TAGS_EXCLUDE smoke -- mm tests/fixtures/passing --tags-exclude extra
```

Explicit tag exclusions replace the environment's excluded tags.

```expected-contains
RUN  basic.md
  RUN  Block 1/2 — Show help
  ✓ PASS Block 1/2 — Show help
  RUN  Block 2/2 — Show the installed version
  ✓ PASS Block 2/2 — Show the installed version
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Enable verbose output from the environment

````markdown markmason
```markmason
mm-test-cli with-env VERBOSE " YES " -- mm tests/fixtures/diagnostics
```

`VERBOSE` accepts `1`, `true`, or `yes`, ignoring case and surrounding spaces.
It shows captured output and the comparison diff, just like `--verbose`.

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
