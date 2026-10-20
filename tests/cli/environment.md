# Set filters in the environment

Environment variables select the same [sample specs](filtering.md) as CLI flags.
[`mm-test-cli with-env`](../support/README.md) sets them for one command, without shell-specific syntax.

## Include paths from the environment

```markmason
mm-test-cli with-env E2E_FILE_PATTERN basic -- mm tests/fixtures/passing
```

Include files whose relative paths match the pattern.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Exclude paths from the environment

```markmason
mm-test-cli with-env E2E_FILE_EXCLUDE basic -- mm tests/fixtures/passing
```

Exclude matching file paths.

```expected-regex
(?s)RUN  extra\.md.*✓ PASS  extra\.md  1/1 blocks.*Blocks: 1 passed, 0 failed
```

## Require tags from the environment

```markmason
mm-test-cli with-env E2E_TAGS SMOKE,cli -- mm tests/fixtures/passing
```

Require every listed tag, ignoring case.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Exclude tags from the environment

```markmason
mm-test-cli with-env E2E_TAGS_EXCLUDE smoke -- mm tests/fixtures/passing
```

Exclude files with any listed tag.

```expected-regex
(?s)RUN  extra\.md.*✓ PASS  extra\.md  1/1 blocks.*Blocks: 1 passed, 0 failed
```

## Choose a different variable prefix

```markmason
mm-test-cli with-env E2E_TAGS extra DOC_TAGS smoke -- mm tests/fixtures/passing --env-prefix DOC_
```

Change the filter prefix. `DOC_TAGS` is used; the default `E2E_TAGS` is ignored.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Override the include pattern

```markmason
mm-test-cli with-env E2E_FILE_PATTERN extra -- mm tests/fixtures/passing --file-pattern basic
```

An explicit file pattern replaces the environment's include pattern.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Override the exclude pattern

```markmason
mm-test-cli with-env E2E_FILE_EXCLUDE basic -- mm tests/fixtures/passing --file-exclude extra
```

An explicit file exclusion replaces the environment's exclusion.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Override required tags

```markmason
mm-test-cli with-env E2E_TAGS extra -- mm tests/fixtures/passing --tags smoke
```

Explicit tags replace the environment's required tags.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Override excluded tags

```markmason
mm-test-cli with-env E2E_TAGS_EXCLUDE smoke -- mm tests/fixtures/passing --tags-exclude extra
```

Explicit tag exclusions replace the environment's excluded tags.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Blocks: 2 passed, 0 failed
```

## Enable verbose output from the environment

```markmason
mm-test-cli with-env VERBOSE " YES " -- mm tests/fixtures/diagnostics
```

`VERBOSE` accepts `1`, `true`, or `yes`, ignoring case and surrounding spaces.
It shows captured output and the comparison diff, just like `--verbose`.

```expected-regex
(?s)RUN  output\.md.*ACTUAL:.*hello.*DIFF \(expected vs actual\):.*✓ PASS Block 1/1 — Print a short result for diagnostics.*Blocks: 1 passed, 0 failed.*Duration:
```
