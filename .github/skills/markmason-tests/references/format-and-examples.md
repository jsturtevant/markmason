# Format and examples

## A normal guide

Use this shape, but replace `mycli`, the version, and output with values observed
in the target project.

````markdown
# Get started with MyCLI

Check which release is installed.

## Check the installed version

Print the installed release.

```markmason
mycli --version
```

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

## Reject unknown options

Reject unknown options without running the command.

```markmason (exit 2)
mycli --not-an-option
```

```expected
error: unknown option --not-an-option
```
````

## Matching and execution

| Syntax | Meaning |
|--------|---------|
| `markmason` fence | Run commands in order as one test; require exit 0 by default |
| `markmason (exit N)` opening fence | Require exit N, without treating execution errors as success |
| `[LANGUAGE] markmason always (exit N)` opening fence | Defer cleanup until the file ends; language and expected exit are optional |
| `expected` fence | Match output after path, whitespace, blank-line, and wrapping normalization |
| `expected-contains` fence | Match whole lines in order; no unmatched trailing lines |
| `expected-substring` fence | Find one literal substring, preserving whitespace, paths, and internal newlines |
| `expected-regex` fence | Search with one Rust regex; use `(?m)` for line anchors or `(?s)` for multiline `.` |
| `stdin` fence | Send its contents plus a final newline |
| `pty` fence | Use paired `match:` / `send:` or `match-raw:` / `send:` steps |

- By default commands run without a shell. Quote arguments with spaces; backslashes are literal.
- Put one command on each line; blank lines are ignored. Combine their output
  for the assertion, and stop at the first nonzero exit.
- Share file state between commands, not shell variables or directory changes.
  Use an explicit interpreter for scripts.
- Do not use shell assignments, redirects, `&&`, or arbitrary environment expansion.
- Pipelines connect stdout and stop at the first failing stage.
- Compare stdout on success; compare stdout followed by stderr on failure.
- Without an expected block, only the exit code is checked.
- Use descriptive headings. Keep input and output blocks before the next heading.
- Wrap literal Markdown examples in a longer fence so their commands do not run.
- PTY `expected` uses contains matching; explicit substring or regex modes are preserved.
  Every scripted step must finish.
- Send `stdin` to the first command only. Advance PTY match/send steps across
  commands in order. Apply timeouts per command or pipeline stage.
- Use `{enter}`, `{up}`, `{down}`, `{space}`, and `{backspace}` for PTY input.
  Sends do not append a newline automatically.

## Partial output assertions

Use these forms only with an `mm` build that supports `expected-substring` and
`expected-regex`. Older builds may ignore these fences and check only exit codes.
Keep `expected` and `expected-contains` for full output or selected whole lines.

For an error such as `Error from server: user "demo-482" is not authorized to xyz`,
keep the command's full output and assert its stable phrase:

````markdown
```expected-substring
is not authorized to xyz
```
````

For output such as `Created pod demo-482 in 1.25s`, match the variable parts:

````markdown
```expected-regex
Created pod demo-\d+ in \d+\.\d+s
```
````

- Treat each block as one substring or pattern, including internal newlines.
  The fence's final newline is not part of the assertion.
- Allow unmatched output before and after; preserve whitespace and paths,
  normalizing only CRLF to LF. Matching is case-sensitive by default.
- Use Rust regex syntax: `(?m)` enables line anchors; `(?s)` lets `.` span newlines.
  Write `\d+` directly, `\.` for a literal dot, and `\\` for a literal backslash.
  Do not add a JSON or shell escaping layer inside the assertion fence.
- Keep `(exit N)` for expected command failures. A text match does not replace
  the exit-code assertion.
- Use a nonempty, meaningful phrase or pattern. Empty assertions match any output.
- Keep missing-match and malformed-pattern examples in a separate error guide.
  A mismatch reports the requested substring or pattern and actual output;
  an invalid regex fails parsing before commands run.

## Deferred cleanup

Use `markmason always` to run cleanup once after selected ordinary blocks finish,
even after a mismatch, timeout, expected or unexpected command failure, or
walkthrough cancellation. Use `bash markmason always` for Markdown language
styling, not interpreter selection; opt into `shell: bash` for shared Bash state.
Keep an expected exit after `always`, for example `markmason always (exit 2)`.

- Keep cleanup safe when earlier work stopped partway through.
- Share the file's fixture and setup state. Skip cleanup if fixture/setup
  creation did not finish.
- Put cleanup blocks in the order they should run. Later cleanup is attempted
  even if an earlier block fails; failures keep the run failed.
- Expect cleanup to run without another walkthrough confirmation. Script any
  PTY input rather than requiring a person to answer a cleanup prompt.
- Select ordinary commands with filters; cleanup-only command matches do not
  select a file. `--list` labels deferred blocks as `Always`.
- Do not assume a terminated Bash session can retain its state. A timed-out or
  interrupted command gets a bounded chance to yield the same session; otherwise
  deferred commands fail explicitly.

  When asserting a nested `mm` run, use the shared presentation vocabulary:
  document `RUN`/`✓ PASS`/`✗ FAIL` lines, `Block N/N — heading` or
  `Always N/N — heading`, `Documents:`, and `Blocks:`. Keep these assertions
  readable as example output; do not collapse them into dense one-line regexes.
  The stable `Blocks:` total is the final summary line, so `expected-contains`
  can skip the dynamic duration without leaving unmatched trailing output.

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
  command blocks or expected output. `--home-env NAME` renames the home variable.
- Leave stdin and PTY text literal; they do not substitute these variables.
- Choose a positive timeout. Frontmatter overrides the command-line default.
- Document prerequisites when selecting a single case would skip earlier state.

## Stateful Bash walkthroughs

Use `shell: bash` when a document needs variables, functions, shell syntax, or
directory changes to persist. Keep commands directly in `markmason` fences,
not in `stdin` scripts or environment-state files.

```yaml
---
shell: bash
setup: mm sharedset.md
---
```

Markdown setup shares the same Bash process and inherits shell mode. It may
also declare `shell: bash` to work standalone. A direct-execution caller must
opt in explicitly before using a Bash prerequisite.

Bash uses `nounset` and `pipefail`, without forced `errexit`. Preserve complete
loops, functions, pipelines, and heredocs; these are logical commands for stepping.
Expected failures retain state, but unexpected failures stop the file.
Use application `stdin` only on the first logical command of a block.

Require Bash on `PATH`: Linux Bash, macOS Bash 3.2+, or native Windows Git Bash.
Do not assume PowerShell, cmd, WSL, or a Bash version newer than the documented
prerequisites. Runner variables expand through Bash; single quotes stay literal.
See [the executable guide](../../../../tests/cli/shell.md).

## Filtering

- Use `--file-pattern` / `--file-exclude` for relative file paths.
- Use `--test-name-pattern` for relative file paths or command text, not headings.
- Select the whole block when any of its commands matches.
- Use Rust regex syntax, for example `'^mm --version$'` for an exact command.
- Use `--tags smoke,cli` for files with both tags; matching ignores case.
- Use `--tags-exclude slow` to reject files with that tag.
- Read `E2E_FILE_PATTERN`, `E2E_FILE_EXCLUDE`, `E2E_TAGS`, and `E2E_TAGS_EXCLUDE`
  unless `--env-prefix` changes the prefix. Explicit flags take precedence.
- Treat no matches as an error, not a successful empty run.
- Use `--fail-fast` only for execution: it stops after the first failed
  document finishes deferred cleanup and reports later selected files as not run.
  Do not combine it with `--list` or `--step`.
- Use `--parallel` or `--parallel N` only for independent files. Commands within
  a file remain ordered. Mark shared-resource files with `parallel: false`; they
  wait for active files and run alone. `--parallel` and `--step` are incompatible.
