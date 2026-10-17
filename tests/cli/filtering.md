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

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## Exclude files by path

```markmason
mm tests/fixtures/passing --file-exclude basic
```

Skip files whose paths match `basic`.

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## Require every listed tag

```markmason
mm tests/fixtures/passing --tags SMOKE,cli
```

Require both tags. Matching ignores case, so `SMOKE` matches `smoke`.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## Exclude a tag

```markmason
mm tests/fixtures/passing --tags-exclude smoke
```

Skip files with the `smoke` tag.

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## Select all commands in a matching file

```markmason
mm tests/fixtures/passing --test-name-pattern basic.md
```

`--test-name-pattern` matches file paths or commands. Here the file name
matches, so both cases run, just as with `--file-pattern basic`.

```expected
basic.md
PASS mm --help
PASS mm --version
pass 2
fail 0
```

## Select individual commands

```markmason
mm tests/fixtures/passing --test-name-pattern "^mm --version$"
```

Here the pattern matches commands instead: only `mm --version` runs in
each file. `--file-pattern` cannot select individual commands.

```expected
basic.md
PASS mm --version
extra.md
PASS mm --version
pass 2
fail 0
```

Combine the flags to narrow files first, then select commands within them.
Earlier cases won't run automatically; select the whole file if cases depend on
each other.

## Combine alternative command patterns

```markmason
mm tests/fixtures/passing --test-name-pattern "^mm --help$" --test-name-pattern "^mm --version$"
```

Repeated name patterns are alternatives: a command only needs to match one.

```expected
basic.md
PASS mm --help
PASS mm --version
extra.md
PASS mm --version
pass 3
fail 0
```

See [environment filters](environment.md) for variables and flag precedence,
or [error cases](errors.md) for filters with no matches.
