# Specification reference

## Commands and output

Write descriptive headings and put commands in `markmason` fenced blocks.
The default expected exit code is 0; add `(exit N)` to the opening fence to
require a different code:

````markdown
## Reject an unknown option

```markmason (exit 2)
mycli --not-an-option
```

```expected
error: unknown option --not-an-option
```
````

Each block is one test. Put one command on each line; blank lines are ignored.
Commands run in order and their output is concatenated for the following
expected block. The first nonzero exit stops the block and becomes its exit
code; if all commands succeed, the block exits 0.
Without an expected block, only the exit code is checked.

Each command starts in the file's working directory and environment, sharing
file state but not shell variables or `cd` changes. Use an explicit interpreter
for shell scripts. A `stdin` block supplies input to the first command only;
later commands start with empty stdin. A `pty` block's match/send steps advance
across commands in order, and must all finish by the end of the block.
Each PTY command uses a separate terminal session; their output is joined on
line boundaries.
Timeouts apply to each command (and each pipeline stage), not the whole block.

Output, stdin, and PTY blocks belong to the preceding `markmason` block until
the next command block or heading. Keep them in the same section.
Both `#` headings and headings underlined with `=` or `-` start a new section.
Ordinary code blocks and examples nested inside longer fences do not run.
Backtick and tilde fences of at least three characters are supported, indented
by at most three spaces; close with the same character and at least the opening
length. Recognized blocks must be closed.
Content loses up to the opening fence's indentation; additional indentation
inside scripts is preserved.

| Block | Behavior |
|-------|----------|
| `expected` | Exact output, normalizing paths, whitespace, blank lines, and terminal wrapping |
| `expected-contains` | Whole lines in order; extra lines between matches are allowed, trailing unmatched lines fail |
| `stdin` | Send the block plus a final newline |
| `pty` | Script an interactive terminal |

- Successful non-PTY commands compare stdout; failures compare stdout followed by stderr.
- Pipelines pass stdout to the next stage and stop at the first nonzero exit.
- Commands do not run through a shell. Quotes group arguments; backslashes stay literal.
- Executables resolve through `PATH`, relative to the working directory.
- `node ./helper.js` resolves the script beside the Markdown file.

## Frontmatter and setup

```yaml
---
setup: python3 setup.py
tags: [smoke, cli]
timeout: 30
---
```

| Field | Meaning |
|-------|---------|
| `setup` | Complete command for this file: Node, Python, or a native executable |
| `clone` | GitHub `owner/repo` to clone with `gh auth status` / `gh repo clone` |
| `tags` | Array of tags for filtering |
| `timeout` | Positive seconds per command; overrides `--timeout` (default 30) |

Setup runs after cloning, in `--cwd`, with the file's isolated environment.
It uses command quoting, not shell expansion or pipelines.
Executables and file arguments found beside the spec resolve there;
explicit `./` and `../` paths also resolve from the spec's directory.
Quote paths with spaces, for example `setup: 'python3 "setup fixtures.py"'`.

## Isolation and cleanup

- Each file gets temporary application-home and XDG configuration directories.
- Cases within a file share state and execute in order.
- Without `clone`, commands run in `--cwd` (default: the current directory).
- Cloned fixtures run in their temporary checkout. Use absolute `PATH` entries
  for local builds so commands remain available after changing directories.
- The caller's GitHub authentication directory is preserved.
- Cleanup removes fixtures after success or failure.
- Unix SIGINT/SIGTERM cancel the process group, clean up, and exit 130/143.
- Windows Job Objects own commands and descendants until file cleanup.
  Successful background jobs survive for later cases; timeouts and execution
  errors terminate the affected job immediately.
- Windows environment overrides are case-insensitive.

## Variables

Command blocks and expected blocks substitute:

| Variable | Value |
|----------|-------|
| `$REPO_ROOT` | Spec's working directory |
| `$PROJECT_NAME` | Working directory's final path component |
| `$XDG_CONFIG_HOME` | Isolated configuration directory |
| `$E2E_HOME` | Isolated application home; rename with `--home-env` |

Stdin and PTY directives remain literal.

## Interactive terminals

````markdown
## Create a project

```markmason
mycli init
```

```pty
match: Project name?
send: demo{enter}
```
```expected-contains
Created demo
```
````

- Terminals are 120 columns by 30 rows.
- `match:` ignores ANSI codes; `match-raw:` preserves them.
- Every match needs a following `send:`. All steps must complete.
- Sends append no newline; use `{enter}` when needed.
- Expected PTY output always uses contains matching.
- Captured output is ANSI-stripped, including timeout and incomplete-script diagnostics.
- Missing prompts, startup failures, and timeouts fail even with an expected nonzero exit.
- Oversized input remains subject to timeout and cancellation.

| Kind | Placeholders |
|------|--------------|
| Input | `{enter}`, `{up}`, `{down}`, `{space}`, `{backspace}` |
| Raw output | `{hide-cursor}`, `{show-cursor}`, `{clear-screen}`, `{alt-screen}`, `\xNN` |

Raw and ANSI-stripped matches advance separate cursors.

## Filtering and diagnostics

| Flag | Environment variable |
|------|----------------------|
| `--file-pattern` | `E2E_FILE_PATTERN` |
| `--file-exclude` | `E2E_FILE_EXCLUDE` |
| `--tags` | `E2E_TAGS` |
| `--tags-exclude` | `E2E_TAGS_EXCLUDE` |

- `--env-prefix` changes the environment prefix. Explicit flags take precedence.
- Tags are comma-separated and case-insensitive. Inclusion requires all tags;
  exclusion rejects any matching tag.
- `--test-name-pattern REGEX` matches a relative spec path or command text, not headings.
  Matching a file runs all its cases; matching any command selects its whole block.
  Selecting a block skips earlier blocks, including their prerequisites.
- File and name patterns use Rust regex syntax.
- `--list` discovers cases without executing setup or creating fixtures.
- No matches, malformed specs, and failed assertions return nonzero exits.
- `--verbose` or `VERBOSE=1` (`true` / `yes` also work) prints output and diffs.
  Failures always include diagnostics.
