# Filter tests

Pick files or individual commands using path patterns, tags, and name filters.

These examples use [two sample specs](../fixtures/passing).
Run from the repository root with `mm` on `PATH`.

| File | Tags | Cases |
|------|------|-------|
| [basic.md](../fixtures/passing/basic.md) | `smoke`, `cli` | 2 |
| [extra.md](../fixtures/passing/extra.md) | `extra` | 1 |

## Select files by path

```markmason
mm tests/fixtures/passing --file-pattern basic
```

`--file-pattern` matches file paths only. Here it runs both cases in
`basic.md`, not individual commands.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Documents: 1 passed, 0 failed.*Blocks: 2 passed, 0 failed
```

## Exclude files by path

```markmason
mm tests/fixtures/passing --file-exclude basic
```

Skip files whose paths match `basic`.

```expected-regex
(?s)RUN  extra\.md.*✓ PASS  extra\.md  1/1 blocks.*Documents: 1 passed, 0 failed.*Blocks: 1 passed, 0 failed
```

## Require every listed tag

```markmason
mm tests/fixtures/passing --tags SMOKE,cli
```

Require both tags. Matching ignores case, so `SMOKE` matches `smoke`.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Documents: 1 passed, 0 failed.*Blocks: 2 passed, 0 failed
```

## Exclude a tag

```markmason
mm tests/fixtures/passing --tags-exclude smoke
```

Skip files with the `smoke` tag.

```expected-regex
(?s)RUN  extra\.md.*✓ PASS  extra\.md  1/1 blocks.*Documents: 1 passed, 0 failed.*Blocks: 1 passed, 0 failed
```

## Select all commands in a matching file

```markmason
mm tests/fixtures/passing --test-name-pattern basic.md
```

`--test-name-pattern` matches file paths or commands. Here the file name
matches, so both cases run, just as with `--file-pattern basic`.

```expected-regex
(?s)RUN  basic\.md.*✓ PASS  basic\.md  2/2 blocks.*Documents: 1 passed, 0 failed.*Blocks: 2 passed, 0 failed
```

## Select individual commands

```markmason
mm tests/fixtures/passing --test-name-pattern "^mm --version$"
```

Here the pattern matches commands instead: only `mm --version` runs in
each file. `--file-pattern` cannot select individual commands.

```expected-regex
(?s)RUN  basic\.md.*Block 1/1 — Show the installed version.*RUN  extra\.md.*Block 1/1 — Show the installed version.*Documents: 2 passed, 0 failed.*Blocks: 2 passed, 0 failed
```

Combine the flags to narrow files first, then select commands within them.
Earlier cases won't run automatically; select the whole file if cases depend on
each other.

## Combine alternative command patterns

```markmason
mm tests/fixtures/passing --test-name-pattern "^mm --help$" --test-name-pattern "^mm --version$"
```

Repeated name patterns are alternatives: a command only needs to match one.

```expected-regex
(?s)RUN  basic\.md.*Block 1/2 — Show help.*Block 2/2 — Show the installed version.*RUN  extra\.md.*Block 1/1 — Show the installed version.*Documents: 2 passed, 0 failed.*Blocks: 3 passed, 0 failed
```

See [environment filters](environment.md) for variables and flag precedence,
or [error cases](errors.md) for filters with no matches.
