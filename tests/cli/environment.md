# Set filters in the environment

Environment variables select the same [sample specs](filtering.md) as CLI flags.
[`mm-test-cli with-env`](../support/README.md) sets them for one command, without shell-specific syntax.

## `mm-test-cli with-env E2E_FILE_PATTERN basic -- mm tests/fixtures/passing`

Include files whose relative paths match the pattern.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env E2E_FILE_EXCLUDE basic -- mm tests/fixtures/passing`

Exclude matching file paths.

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## `mm-test-cli with-env E2E_TAGS SMOKE,cli -- mm tests/fixtures/passing`

Require every listed tag, ignoring case.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env E2E_TAGS_EXCLUDE smoke -- mm tests/fixtures/passing`

Exclude files with any listed tag.

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## `mm-test-cli with-env E2E_TAGS extra DOC_TAGS smoke -- mm tests/fixtures/passing --env-prefix DOC_`

Change the filter prefix. `DOC_TAGS` is used; the default `E2E_TAGS` is ignored.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env E2E_FILE_PATTERN extra -- mm tests/fixtures/passing --file-pattern basic`

An explicit file pattern replaces the environment's include pattern.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env E2E_FILE_EXCLUDE basic -- mm tests/fixtures/passing --file-exclude extra`

An explicit file exclusion replaces the environment's exclusion.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env E2E_TAGS extra -- mm tests/fixtures/passing --tags smoke`

Explicit tags replace the environment's required tags.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env E2E_TAGS_EXCLUDE smoke -- mm tests/fixtures/passing --tags-exclude extra`

Explicit tag exclusions replace the environment's excluded tags.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## `mm-test-cli with-env VERBOSE " YES " -- mm tests/fixtures/diagnostics`

`VERBOSE` accepts `1`, `true`, or `yes`, ignoring case and surrounding spaces.
It shows captured output and the comparison diff, just like `--verbose`.

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
