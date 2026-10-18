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
| `stdin` | Send the block plus a final newline |
| `pty` | Script an interactive terminal |

- Successful non-PTY commands compare stdout; failures compare stdout followed by stderr.
- In direct-execution mode, pipelines pass stdout to the next stage and stop at the first nonzero exit.
- Unless `shell: bash` is set, commands do not run through a shell. Quotes group arguments; backslashes stay literal.
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
forward slashes for Bash and native-tool interoperability. Other inherited values,
including `KUBECONFIG`, remain unchanged. Commands use real Bash expansion rather
than textual replacement, so single quotes and escaped dollars work normally.
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
  KUBECONFIG: Path to the kubeconfig for the disposable cluster
  STORAGE_CONFIG: Name of an existing storage configuration
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
- Expected PTY output always uses contains matching.
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
