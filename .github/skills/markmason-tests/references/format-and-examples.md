# Format and examples

## A normal guide

Use this shape, but replace `mycli`, the version, and output with values observed
in the target project.

````markdown
# Get started with MyCLI

Check which release is installed.

## `mycli --version`

Print the installed release.

```expected
mycli 1.2.3
```
````

## A separate error guide

Keep deliberate failures out of the normal reading path. Do not assume another
CLI uses this exit code or wording; record its real behavior.

````markdown
# Argument errors

These commands deliberately fail. They show how invalid arguments are reported.

## `mycli --not-an-option` (exit 2)

Reject unknown options without running the command.

```expected
error: unknown option --not-an-option
```
````

## Matching and execution

| Syntax | Meaning |
|--------|---------|
| ``## `command` `` | Execute a command; require exit 0 by default |
| `(exit N)` after the command | Require exit N, without treating execution errors as success |
| `expected` fence | Match output after path, whitespace, blank-line, and wrapping normalization |
| `expected-contains` fence | Match whole lines in order; no unmatched trailing lines |
| `stdin` fence | Send its contents plus a final newline |
| `pty` fence | Use paired `match:` / `send:` or `match-raw:` / `send:` steps |

- Commands run without a shell. Quote arguments with spaces; backslashes are literal.
- Do not use shell assignments, redirects, `&&`, or arbitrary environment expansion.
- Pipelines connect stdout and stop at the first failing stage.
- Compare stdout on success; compare stdout followed by stderr on failure.
- Without an expected block, only the exit code is checked.
- PTY output uses contains matching. Every scripted step must finish.
- Use `{enter}`, `{up}`, `{down}`, `{space}`, and `{backspace}` for PTY input.
  Sends do not append a newline automatically.

## Frontmatter and state

```yaml
---
setup: python3 setup.py
tags: [smoke, cli]
timeout: 30
---
```

- Treat `setup` as a complete per-file command, not a script with an assumed runtime.
- Resolve setup file arguments beside the spec. Setup runs in `--cwd`, after cloning.
- Use `clone: owner/repo` only when needed; it requires authenticated GitHub CLI.
- Give each file its own temporary home and XDG config. Cases within it share state.
- Use `$REPO_ROOT`, `$PROJECT_NAME`, `$XDG_CONFIG_HOME`, and `$E2E_HOME` in
  command headings or expected output. `--home-env NAME` renames the home variable.
- Leave stdin and PTY text literal; they do not substitute these variables.
- Choose a positive timeout. Frontmatter overrides the command-line default.
- Document prerequisites when selecting a single case would skip earlier state.

## Filtering

- Use `--file-pattern` / `--file-exclude` for relative file paths.
- Use `--test-name-pattern` for relative file paths or command headings.
- Use Rust regex syntax, for example `'^mm --version$'` for an exact command.
- Use `--tags smoke,cli` for files with both tags; matching ignores case.
- Use `--tags-exclude slow` to reject files with that tag.
- Read `E2E_FILE_PATTERN`, `E2E_FILE_EXCLUDE`, `E2E_TAGS`, and `E2E_TAGS_EXCLUDE`
  unless `--env-prefix` changes the prefix. Explicit flags take precedence.
- Treat no matches as an error, not a successful empty run.
