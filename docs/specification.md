# Specification reference

This specification reference may help users who need exact
behavior, but is primarily written for agents and contributors developing
Markmason itself.  See the [docs to learn and see example usage](../tests/cli/cli.md)

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

Each block is one test. By default, put one command on each line; blank lines are ignored.
Commands run in order and their output is concatenated for the following
expected block. The first nonzero exit stops the block and becomes its exit
code; if all commands succeed, the block exits 0.
Without an expected block, only the exit code is checked.

Without `shell: bash`, each command starts in the file's working directory and environment, sharing
file state but not shell variables or `cd` changes. Use an explicit interpreter
for shell scripts. A `stdin` block supplies input to the first command only;
later commands start with empty stdin. A `pty` block's match/send steps advance
across commands in order, and must all finish by the end of the block.
In default direct-execution mode, each PTY command uses a separate terminal session; their output is joined on
line boundaries.
Timeouts apply to each command (and each pipeline stage), not the whole block.

Output, stdin, and PTY blocks belong to the preceding `markmason` block until
the next command block or heading. Keep them in the same section.
Both `#` headings and headings underlined with `=` or `-` start a new section.
Ordinary code blocks and examples nested inside longer fences do not run.
Markdown follows CommonMark structure, including lists, block quotes, and HTML.
Literal fenced examples inside those containers remain documentation, not tests.
Backtick and tilde fences of at least three characters are supported, indented
by at most three spaces; close with the same character and at least the opening
length. Recognized blocks must be closed.
Content loses up to the opening fence's indentation; additional indentation
inside scripts is preserved.
For command blocks in lists or block quotes, Markdown container prefixes are
also removed from their contents.

| Block | Behavior |
|-------|----------|
| `expected` | Exact output, normalizing paths, whitespace, blank lines, and terminal wrapping |
| `expected-contains` | Whole lines in order; extra lines between matches are allowed, trailing unmatched lines fail |
| `expected-substring` | One literal substring anywhere in output, including internal newlines |
| `expected-regex` | One Rust regex searched anywhere in output |
| `stdin` | Send the block plus a final newline |
| `pty` | Script an interactive terminal |

- Successful non-PTY commands compare stdout; failures compare stdout followed by stderr.
- In direct-execution mode, pipelines pass stdout to the next stage and stop at the first nonzero exit.
- Unless `shell: bash` is set, commands do not run through a shell. Quotes group arguments; backslashes stay literal.
- Executables resolve through `PATH`, relative to the working directory.
- `node ./helper.js` resolves the script beside the Markdown file.

### Partial output assertions

Use `expected-substring` for a stable phrase inside a longer line, without
filtering away the command's real output:

````markdown
```markmason (exit 1)
mycli create demo
```

```expected-substring
is not authorized to xyz
```
````

Use `expected-regex` when the assertion itself needs a variable part:

````markdown
```expected-regex
Created pod demo-\d+ in \d+\.\d+s
```
````

Both search the whole captured output and allow unmatched text before and after
the match. The block is one substring or pattern, not a list of independent
assertions. Matching is case-sensitive by default. Unlike `expected` and
`expected-contains`, these forms preserve whitespace, blank lines, path
separators, and wrapping; only CRLF line endings become LF. The closing fence's
preceding newline is not part of the assertion.

Regexes use [Rust regex syntax](https://docs.rs/regex/latest/regex/#syntax):
`(?m)` enables line anchors, `(?s)` lets `.` match newlines, and `\A` / `\z`
anchor the entire output. Write escapes directly, for example `\d+` or
`report\[1\]\.txt`; a literal backslash is `\\`. Lookaround and backreferences
are not supported. Invalid patterns fail spec parsing before commands run.
Runner variable substitution still applies; substituted values are not
automatically regex-escaped and must leave a valid pattern.

Mismatches report the missing substring or pattern and the actual output,
rather than an exact-output diff. Empty substrings and patterns match any
output. See the [runnable matching examples](../tests/cli/matching.md) and
[deliberate failures](../tests/cli/matching-errors.md).

## Deferred cleanup

Add `always` immediately after `markmason` to defer a block until the file's
ordinary execution ends:

````markdown
```bash markmason always
kubectl delete namespace "$NS" --ignore-not-found
```
````

The language tag is optional Markdown styling metadata; it does not select an
interpreter. Use `shell: bash` for Bash expansion and shared variables/functions.
The one supported modifier order is `[LANGUAGE] markmason always (exit N)`,
with the language and expected exit both optional. Output, stdin, and PTY
assertions work as for ordinary blocks.

- Selected ordinary blocks run first. All of that file's `always` blocks then
  run once in document order, even if they appeared earlier in the document.
- Cleanup shares the fixture, environment, working directory, cloned checkout,
  and Bash session, including Markdown setup state. Cleanup from successful
  Markdown prerequisites is deferred too, in setup-completion order before
  the caller's cleanup.
- Cleanup runs after success, assertion/command failures, spawn errors, timeouts,
  walkthrough `q` or EOF, and catchable Ctrl+C/SIGTERM. It is skipped if inputs,
  fixture creation, or setup did not complete. Uncatchable termination cannot
  run cleanup.
- Every deferred block is attempted even when an earlier one fails. Failures
  remain failures; cleanup cannot turn a failed run into success. Successful
  cleanup blocks contribute to the pass count, and failures to the fail count.
- During cleanup, the original cancellation remains the final exit status.
  Another interrupt can stop the current cleanup command; later deferred blocks
  are still attempted. Normal per-command timeouts apply.
- A Bash timeout or interrupt gets a bounded two-second opportunity to return
  to the same session. Existing SIGINT handlers and explicitly ignored signals
  are preserved; a recovery fallback is installed only when no handler is set.
  If Bash has exited, been replaced with `exec`, or cannot
  resume safely, deferred commands report the closed session rather than
  starting a new shell with missing state.
- `--list` labels deferred commands as `Always` and requires no inputs or
  fixture. Name filters select ordinary blocks only; once one is selected,
  all cleanup blocks in that file run. Matching only cleanup commands selects
  nothing. A file containing only `always` blocks has no runnable ordinary test.
- Walkthrough cleanup runs automatically outside the fullscreen view, with
  the same `Always N/N — heading` identity as other views. It never asks
  for another walkthrough confirmation. PTY steps must still be fully scripted.

Keep cleanup safe when ordinary work stopped halfway through. See the
[runnable guide](../tests/cli/cleanup.md).

## Run progress and summaries

Markmason uses Markdown document paths and block headings as the shared identity
in normal runs, `--list`, and `--step`. A block without a heading falls back to
its first command, truncated for display only; diagnostics retain the full
command and output.

Redirected output is append-only:

```text
RUN  getting-started.md
  RUN  Block 1/2 — Create the project
  ✓ PASS Block 1/2 — Create the project
✓ PASS  getting-started.md  2/2 blocks  <1s
```

Interactive terminals show the active document once and update a cyan spinner
line in place with the current `Block N/N — heading` and elapsed time. The
document line has its own elapsed wall-time value, right-aligned to the same
terminal column as the block timer. Document time starts before inputs/setup and
includes setup, ordinary blocks, user pauses in `--step`, deferred cleanup, and
fixture cleanup. It resets for the next file; the block timer resets for each
block. Completed
passing ordinary blocks do not add noise; document completion prints one green
`✓ PASS` line. Failures clear the spinner and print a red `✗ FAIL` block identity,
the full first command, and the existing mismatch, output, timeout, spawn, setup,
terminal, deferred-cleanup, or fixture-cleanup diagnostic.

The final summary separates documents from blocks, lists every passing and
failing document, and lists failed headings beneath their document. Deferred
`always` blocks are included in the explicit block totals. Setup, terminal, and
fixture cleanup errors fail the document without pretending to be Markdown
blocks. Duration appears before the stable final `Blocks:` line so executable
guides can use readable whole-line assertions without hardcoding timing.
Quitting a walkthrough produces a yellow cancelled document and keeps
the historical successful exit when no real failure occurred.

`--list` groups blocks by document, labels them with their execution position,
heading, and `Block` or `Always`, then prints their commands without setup,
inputs, or fixture creation. `--step` uses the same path, `N/N`, heading fallback,
`✓ PASS`, and `✗ FAIL` vocabulary inside its fullscreen view, followed by the
shared run summary.

Colors follow terminal detection and `NO_COLOR`; redirected output contains no
ANSI color sequences. Long Unicode paths and headings are display-width
truncated in live terminal status lines without changing diagnostics or the
right-aligned timers. Very narrow terminals keep separate document and block
lines and clamp each line to the available display width.
`--verbose` retains its existing meaning: print captured output and comparison
details inside the same document/block boundaries.

## Fail-fast execution

Use `--fail-fast` to stop scheduling documents after the first selected
document fails. The default remains a complete suite run.

- The current document reaches its normal boundary first: every deferred
  `markmason always` block is attempted and fixture cleanup completes.
- Setup, ordinary block, assertion, timeout, spawn, terminal, deferred cleanup,
  and fixture cleanup failures trigger the stop. Successfully matched expected
  nonzero exits do not.
- Unstarted selected documents appear under yellow `NOT RUN` in the final
  summary. They do not count as failed blocks or documents.
- File, tag, and command filters apply before fail-fast scheduling.
- `--fail-fast` is rejected with `--list` and `--step`, where it has no useful
  serial-suite meaning.
- Cancellation retains its signal exit and cancelled-document terminology;
  it is not reported as a fail-fast failure.

See the [executable running guide](../tests/cli/running.md).

## Bounded file parallelism

Use `--parallel` to run independent Markdown files concurrently. Bare
`--parallel` uses `std::thread::available_parallelism()` with a fallback of one;
`--parallel N` uses a positive explicit limit up to 256. Without the flag,
execution remains serial.

- Only files are parallelized. Setup, commands, blocks, PTY steps, and deferred
  cleanup inside one file remain strictly ordered.
- Every file keeps its own fixture, application home, configuration, clone,
  Bash session, timeout, setup chain, and cleanup lifecycle.
- File output is buffered and printed in selected path order after workers
  finish, so diagnostics never interleave. The final summary uses the same
  deterministic order.
- `parallel: false` in frontmatter makes a file a serial barrier: active files
  finish first, the serial file runs alone, and later files start afterward.
- `--parallel` is rejected with `--step`. `--list` remains non-executing and
  accepts the flag without creating workers.
- With `--fail-fast`, no new files are scheduled after the first observed
  failed document. Files already running reach their normal boundary, including
  deferred cleanup. If several in-flight files fail, all failures are reported
  in path order; files never started are listed as not run.
- Cancellation stops scheduling, lets active files process the shared signal
  and cleanup safely, and leaves queued files not run.
- Authors remain responsible for unique ports, external paths, database names,
  namespaces, accounts, and other resources outside Markmason's isolated
  fixture directories.

Zero, negative, malformed, and explicit values above 256 are rejected.
See the [executable running guide](../tests/cli/running.md).

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
| `shell` | Optional `bash`; keep one Bash session for this file and its Markdown prerequisites |
| `setup` | Complete command, or `mm file.md` to reuse a Markdown setup document |
| `clone` | GitHub `owner/repo` to clone with `gh auth status` / `gh repo clone` |
| `tags` | Array of tags for filtering |
| `timeout` | Positive seconds per command; overrides `--timeout` (default 30) |

Setup runs after cloning, in `--cwd`, with the file's isolated environment.
It uses command quoting, not shell expansion or pipelines.
Executables and file arguments found beside the spec resolve there;
explicit `./` and `../` paths also resolve from the spec's directory.
Quote paths with spaces, for example `setup: 'python3 "setup fixtures.py"'`.

To prepare a walkthrough using a getting-started document, write:

```yaml
---
setup: mm gettingstarted.md
---
```

`mm file.md` (also `mm.exe file.md` on Windows) in setup is a Markdown
prerequisite: the runner executes that file's setup, commands, and assertions
in the caller's existing fixture, rather than starting an isolated child runner.
The path resolves beside the calling file; quote paths containing spaces.
Application home, configuration, working directory, variables, and background
processes are shared. A prerequisite's external setup also runs in that shared
working directory, including a cloned checkout.
Setup commands do not pause in `--step` mode or count
toward the walkthrough's block progress.

Prerequisites run in full regardless of the caller's filters. Their own `timeout`
overrides the caller's timeout. Failure stops setup immediately and skips the
calling file's commands. Further Markdown prerequisites are supported; cycles
are errors. Put `clone` on the calling file, not a shared setup document.
Other setup commands continue to run as external executables.
In Bash mode, external setup commands execute inside the shared shell instead,
so `setup: source "preset env.sh"` can retain variables and functions.
Setup argument quoting and spec-relative path resolution still apply; the
frontmatter value itself does not gain shell expansion or pipelines.
`--list` does not execute or expand prerequisites.

See the executable [setup example](../tests/cli/state.md) and its
[getting-started document](../tests/fixtures/markdown-setup/gettingstarted.md).

## Persistent Bash sessions

Opt in with `shell: bash` on the calling file:

````markdown
---
shell: bash
setup: mm sharedset.md
---

```markmason
export MESSAGE="hello"
for word in one two; do
  printf '%s: %s\n' "$MESSAGE" "$word"
done
```
```expected
hello: one
hello: two
```
````

One real Bash process owns assignments, exports, functions, shell options,
and directory changes across blocks and Markdown setup. The previous
user-command status (`$?`), last argument (`$_`), and options such as `noclobber` survive runner
bookkeeping too. A prerequisite inherits its caller's shell mode; it can also
declare `shell: bash` to work standalone.
A Bash prerequisite cannot silently change a direct-execution caller's mode:
add `shell: bash` to the caller too. Independently selected files never share
shell sessions.

The shell starts with `nounset` and `pipefail`, but not `errexit`. Markmason checks
each top-level logical command's status; compound commands retain normal Bash
semantics. Bash pipelines return the rightmost failing stage's status.
An expected nonzero exit ends its block without destroying the session.
An unexpected exit code, assertion failure, shell exit (`exit` or `exec`), or
timeout stops the rest of that file. The runner never silently restarts Bash.

Write ordinary Bash syntax directly: substitutions, redirects, `&&`, loops,
functions, heredocs, multiline pipelines, indentation, and blank lines are
preserved. Complete top-level shell constructs are individual commands, including
semicolon-separated commands. With `--step`, a complete loop or function
definition gets one pause, not one per physical line. Syntax is parsed before
execution; Bash itself still determines which features its installed version supports.

Commands are sourced separately. Interpreter metadata such as `BASH_SOURCE`,
`LINENO`, and `PIPESTATUS` reflects that transport, not one monolithic Markdown
script. Keep a pipeline and inspection of its `PIPESTATUS` in the same compound
command. Names beginning with `__markmason_` are reserved for the runner.
Standard tracing options persist, with runner bookkeeping suppressed from
stderr tracing. Interpreter hooks and custom trace descriptors can observe
the sourced-command transport.

The shell is noninteractive and does not load profiles, `.bashrc`, `BASH_ENV`,
or `ENV`, and ignores inherited `SHELLOPTS`/`BASHOPTS` so its starting options
are predictable. It requires `bash`, `stty`, and `sleep` on `PATH`.
Linux uses the installed Bash; macOS supports built-in Bash 3.2 regardless of
the user's login shell. Native Windows requires a local Git Bash installation on
`PATH`, alongside the native tools being tested. A WSL launcher is not a native
Bash replacement; run Linux `mm` and Linux tools inside WSL instead.
No PowerShell, cmd, or other-shell fallback is attempted.

Runner-owned path variables are exported to Bash; on Windows their values use
forward slashes for Bash and native-tool interoperability. Other inherited
values remain unchanged. Commands use real Bash expansion rather than textual
replacement, so single quotes and escaped dollars work normally.
Expected blocks retain the usual runner variable substitution.

`stdin` remains application input for the first logical command only; later
commands get EOF. It never carries runner control messages or walkthrough input.
Commands with `pty` share the file's live terminal and shell state; commands
without `pty` still capture stdout/stderr separately using the usual matching
rules. Quitting, cancellation, failure, and normal cleanup terminate the shell
and its owned descendants before removing fixtures.

See the [executable shell guide](../tests/cli/shell.md) and the
[terminal-driven shell walkthrough](../tests/cli/stepping.md).

## Isolation and cleanup

- Each file gets temporary application-home and XDG configuration directories.
- Cases within a file share state and execute in order.
- Without `clone`, commands run in `--cwd` (default: the current directory).
- Cloned fixtures run in their temporary checkout. Use absolute `PATH` entries
  for local builds so commands remain available after changing directories.
- The caller's GitHub authentication directory is preserved.
- Cleanup removes fixtures after success or failure.
- Unix SIGINT/SIGTERM cancel the process group, clean up, and exit 130/143.
- On Linux and macOS, terminal cleanup also terminates other process groups
  in the owned terminal session, including jobs started with Bash `set -m`.
- Windows Job Objects own commands and descendants until file cleanup.
  Successful background jobs survive for later cases; timeouts and execution
  errors terminate the affected job immediately.
- Windows environment overrides are case-insensitive.

## Variables

In direct-execution mode, command blocks and expected blocks substitute:

| Variable | Value |
|----------|-------|
| `$REPO_ROOT` | Spec's working directory |
| `$PROJECT_NAME` | Working directory's final path component |
| `$XDG_CONFIG_HOME` | Isolated configuration directory |
| `$E2E_HOME` | Isolated application home; rename with `--home-env` |

Stdin and PTY directives remain literal.
In Bash mode these values are environment variables in commands; only expected
blocks use textual substitution.

## Walkthrough mode

Use `--step` to turn the same specs into a guided tutorial:

```sh
mm tests/cli --step
```

A searchable picker lists the files matching your filters. Use the arrow keys
or type to search, then press **Enter** to choose one file. **Esc** cancels before
any setup, commands, or fixtures run. This also works when only one file matches.

To skip the picker, pass a Markdown file directly or select files with
`--file-pattern` (or its environment variable). Without an interactive terminal,
one of those explicit selections is required:

```sh
mm tests/cli --step --file-pattern '^greet\.md$'
mm tests/cli/greet.md --step
```

The walkthrough shows a Markdown-style section with its heading, descriptive
prose, complete code block, expected output, and a collecting actual-output area.
At each pause, `> [next]` points to the next command to run. Completed commands
retain a green `[done]` prefix; running and failed commands are also marked at
the front. Bash loops, functions, pipelines, and heredocs still
execute as complete logical commands, not isolated physical lines.

Supported terminals redraw the current block in an alternate screen, restoring
the original screen on finish, quit, or cancellation. The section is one
scrollable document: no collapsed panes, hidden-line summaries, or truncated
output. Long lines wrap without dropping text. Use **Up/Down**, **PgUp/PgDn**,
**Home/End**, or the **mouse wheel** while paused; scrolling does not execute
commands or require Enter. Keep tutorial sections short so they normally fit.
Tabs are visually expanded; assertions always use complete, unmodified output.
Very small terminals, redirected output, and `TERM=dumb` produce full,
append-only snapshots. Colors are automatic; `NO_COLOR=1` disables color without
disabling the reusable view.

- After file selection, setup and the first command run immediately. After each
  command, **Enter** advances to the next command or block. The final pause lets
  you inspect the last result before finishing.
- **b then Enter** runs the remaining commands in the current block without
  pauses and checks its assertion. It pauses at the next block **before** its
  first command runs, with a fresh output area. From that ready state, Enter
  starts single stepping, or b runs that entire block.
- **f then Enter** runs the rest of the current file without pauses, retaining
  every assertion. Automatic execution stops for failure inspection; a later
  file starts paused rather than inheriting this mode. Ctrl+C still cancels
  while commands are running.
- **q then Enter** quits. Closed input also stops; neither runs the remaining
  commands. Temporary fixtures stay available while paused and are cleaned up
  when you finish, quit, or cancel.
- Within a multi-command block, expected output applies to the whole block.
  Intermediate pauses show actual output collected so far; the assertion runs
  only when the block finishes or a command exits nonzero.
- A pipeline is one command for stepping. App stdin and scripted PTY input
  remain separate from the walkthrough's Enter/quit controls.
- Failures still pause for inspection. Continue to the next block or quit;
  any recorded failure keeps the final exit status nonzero. Quitting without
  failures exits zero, but the summary says **stopped**, not finished, and
  unfinished blocks are not counted as passed.
- In Bash mode, an unexpected failure instead shows **Stop this file**:
  later blocks cannot safely continue with potentially damaged shell state.
- File, tag, and command-name filters behave as usual. A matching command
  selects its whole block; earlier prerequisite blocks are not added.
  `--step` and `--list` cannot be combined.

Try the [executable walkthrough](../tests/cli/walkthrough.md) in this checkout.

## Required environment inputs

Declare an ordered mapping of environment names to plain-text questions in
frontmatter:

```yaml
inputs:
  TUTORIAL_NAME: Name to display in the walkthrough
  OUTPUT_FORMAT: Output format for the examples
```

- Names must match `[A-Za-z_][A-Za-z0-9_]*`, and questions must be nonempty
  strings. Structured defaults and implicit detection of arbitrary shell
  variables are not supported.
- Nonempty caller environment values are reused without prompting. Missing or
  empty values prompt only with `--step`; unattended runs list all missing
  names and fail before cloning, fixtures, or setup commands.
- Requirements in recursive Markdown setup are collected first as well. A
  name declared more than once prompts only once, using the root declaration
  first. Windows environment names are case-insensitive.
- Pressing Enter on an empty answer asks again. `q` then Enter, EOF, or Ctrl+C
  cancels before setup. Values are passed as literal environment data, never
  evaluated as shell commands, and do not leak into subsequent files or the
  calling terminal.
- Inputs are ordinary environment variables, not additional direct-mode
  substitution placeholders. Use `shell: bash` for `$NAME` expansion in
  commands. The existing fixture placeholders keep their original behavior.
- `--list` does not prompt or require values. Runner-managed environment names
  such as `REPO_ROOT`, `PROJECT_NAME`, `XDG_CONFIG_HOME`, `GH_CONFIG_DIR`, and the
  configured application home are rejected rather than silently overridden.
  `BASH_ENV`, `ENV`, `SHELLOPTS`, `BASHOPTS`, `TERM`, and the reserved
  `__markmason_` prefix are also protected.
- On Windows, input declarations that differ only in capitalization are rejected
  before prompting or setup. Use identical spelling across prerequisites and
  Bash commands; Bash variable lookup is case-sensitive even on Windows.
- These are visible, plain-text prompts: do not use them for secrets.

See the [real-terminal input tests](../tests/cli/inputs.md).

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
- PTY `expected` and `expected-contains` blocks use ordered whole-line matching.
  Explicit `expected-substring` and `expected-regex` blocks keep their chosen mode.
- Captured output is ANSI-stripped with terminal line boundaries preserved,
  including timeout and incomplete-script diagnostics.
- Missing prompts, startup failures, and timeouts fail even with an expected nonzero exit.
- Oversized input remains subject to timeout and cancellation.

| Kind | Placeholders |
|------|--------------|
| Input | `{enter}`, `{up}`, `{down}`, `{space}`, `{backspace}`, `{escape}`, `{ctrl-c}` |
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
- Outside walkthrough mode, `--verbose` or `VERBOSE=1` (`true` / `yes` also work)
  prints output and diffs.
  Failures always include diagnostics.
