# Markmason

Markdown-driven CLI tests in a single Rust binary: **`mm`**. Write commands and
expected output in Markdown, then run them without a JavaScript test framework.
Interactive terminals, pipelines, isolated fixtures, and per-file setup commands
are built in. No Rust toolchain or Node runtime is required to run the binary;
your tested commands and setup scripts bring their own runtime requirements.

## Install

Download an archive from [Releases](https://github.com/jsturtevant/markmason/releases),
verify it against `SHA256SUMS`, and put `mm` (Windows: `mm.exe`) on `PATH`.

| Platform | Target | Archive |
|----------|--------|---------|
| Linux x64, glibc 2.35+ | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `.zip` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |

For example, with GitHub CLI installed:

```sh
gh release download v0.1.0 --repo jsturtevant/markmason \
  --pattern 'markmason-v0.1.0-x86_64-unknown-linux-gnu.tar.gz*'
sha256sum --check markmason-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf markmason-v0.1.0-x86_64-unknown-linux-gnu.tar.gz mm
mkdir -p ~/.local/bin
install -m 755 mm ~/.local/bin/mm
mm --version
```

Ensure `~/.local/bin` is on `PATH`. On macOS use `shasum -a 256 -c`
instead of `sha256sum --check`.

Or build from source with Rust 1.88+:

```sh
cargo install --git https://github.com/jsturtevant/markmason --tag v0.1.0 --locked
```

## Run

Commands execute exactly as written and resolve through `PATH`. Put the tested
binary on `PATH`; there is no application-specific executable substitution:

```sh
PATH="$PWD/target/debug:$PATH" mm tests/cli \
  --home-env MYCLI_HOME
```

The spec directory defaults to `tests/cli`. Use `mm --help` for all options.
For example, `mm tests/cli --list` lists cases without executing them.

Relative executables resolve from each spec's working directory; use absolute
paths for local builds when specs clone fixtures. `node ./helper.js` resolves
the script relative to the spec file, preserving the existing format.

Each file gets independent temporary application-home and XDG config directories.
Tests within a file run sequentially and share those directories. Without
`clone` frontmatter, commands run in `--cwd` (default: the current directory).
With `clone`, `gh auth status` and `gh repo clone` prepare a temporary repository.
The caller's GitHub authentication directory is preserved across XDG isolation.
Fixtures are cleaned up after each file, including failures. On Unix, SIGINT
and SIGTERM cancel the active command's process group, reap the command, clean
up fixtures, and stop the suite (exit 130 and 143 respectively).
On Windows, commands and PTYs run in Job Objects. A startup gate establishes
job ownership before the tested command can spawn descendants. Successful jobs
remain owned by the Markdown file so intentionally backgrounded agents can
finish work needed by later cases. File cleanup terminates these jobs before
removing fixtures. Timeouts and command-execution errors terminate their jobs
immediately, even if the original command has already exited. Relay startup
failures are execution errors, not expected command exit codes.
Fixture environment overrides are case-insensitive on Windows.

## Specification format

````markdown
---
tags: [smoke]
timeout: 30
---
# My CLI

## `mycli --help`
```expected
Usage: mycli
```

## `mycli unknown` (exit 1)
```expected-contains
Error: unknown command
```
````

Optional YAML fields: `clone: owner/repo`, `setup: COMMAND`, `tags: [tag, ...]`,
and positive `timeout` in seconds. `setup` is a complete command local to that
Markdown file, not a suite-wide interpreter setting:

```yaml
setup: node setup-squad.js
```

Another file can declare `setup: python3 setup.py`, or execute a native script
directly with `setup: ./setup`. Quoting supports arguments and paths with spaces,
for example `setup: 'python3 "setup fixtures.py" --mode test'`.

Setup uses the same argument quoting rules as test commands, with no shell
expansion or pipeline processing. Executables resolve through `PATH` unless
they name a file beside the spec. Relative file arguments that exist beside the
spec, and explicit `./` or `../` paths, resolve relative to the Markdown file.
Setup runs from `--cwd`, after fixture cloning, with the file's isolated
application-home and configuration environment. Its command and timeout apply
only to that file. There is no `--setup-runner` flag.

Headings use ``## `command` `` with optional `(exit N)` (default exit 0).
Supported fenced blocks:

| Block | Meaning |
|-------|---------|
| `expected` | Exact output after normalizing paths, whitespace, blank lines and terminal wrapping |
| `expected-contains` | Expected lines must appear **as whole lines**, in order; trailing unmatched lines fail |
| `stdin` | Pipe the block plus a final newline to the command |
| `pty` | Interactive `match:` / `match-raw:` followed by `send:` directives |

Without an expected block, only the exit code is checked. Successful non-PTY
commands compare stdout; unsuccessful commands compare stdout followed by stderr.
Pipelines connect stdout to the next stage's stdin and stop at the first nonzero
exit. Commands never run through a shell. Single/double quotes group arguments;
backslashes are literal, with no shell expansion.

PTYs are 120 columns by 30 rows. `match:` ignores ANSI codes; `match-raw:`
preserves them. Matching advances separate raw and clean cursors. Every scripted
step must complete; missing prompts, spawn errors and timeouts always fail, even
when a nonzero exit is expected. PTY expected output uses contains matching.
Oversized scripted input remains subject to the command timeout and cancellation,
even when the child stops reading.

`send:` supports `{enter}`, `{up}`, `{down}`, `{space}`, `{backspace}`; no newline
is appended automatically. Raw matching supports `{hide-cursor}`, `{show-cursor}`,
`{clear-screen}`, `{alt-screen}`, and `\xNN`.

Command headings and expected output substitute `$REPO_ROOT`, `$PROJECT_NAME`,
`$XDG_CONFIG_HOME`, and the configured application-home variable (`$E2E_HOME`
by default). Stdin and PTY directive text remain literal.

## Filtering and diagnostics

Use `--file-pattern`, `--file-exclude`, `--tags`, `--tags-exclude`, or their
environment counterparts `E2E_FILE_PATTERN`, `E2E_FILE_EXCLUDE`, `E2E_TAGS`,
`E2E_TAGS_EXCLUDE`. `--env-prefix` changes the environment prefix. Explicit
flags take precedence. Tags are comma-separated and case-insensitive; inclusion
requires all tags, exclusion rejects any matching tag.

`--test-name-pattern REGEX` matches a relative spec path or command heading.
Matching a file selects every command in that file. Selecting a single command
does **not** run earlier prerequisite commands. Filters use Rust regex syntax.
`--list` lists selected commands without running setup or creating fixtures.
No matches and malformed specs are errors.

`--verbose` or `VERBOSE=1` (`true` and `yes` also work) prints output and diffs.
Failures always include diagnostics and produce a nonzero process exit, even
when name-filtered. `--timeout SECONDS` changes the default 30-second timeout;
frontmatter overrides it.

## Development

Install Rust with `rustfmt` and `clippy`, then install
[Just](https://github.com/casey/just) with `cargo install --locked just`.

```sh
just check    # Formatting, Clippy, unit and integration tests
just fmt      # Apply Rust formatting
just package x86_64-unknown-linux-gnu preview-release
```

CI runs the same checks and packages native binaries on Linux, Windows, and both
macOS architectures. Packaging requires Bash and standard archive/checksum tools
(7-Zip on Windows). Artifacts go into `dist/`.

Tests compile a small Rust fixture executable and exercise pipelines, stdin,
PTYs, filters, setup, cloned fixtures, isolation, cleanup and failure handling
without Node, GitHub authentication, or network access.
Unix blocked-input regression tests use `stty` to put the fixture PTY in raw mode.

## Releases

Successful pushes to `main` update the rolling **`preview-release`** prerelease,
including its tag and binary assets. Preview assets are mutable; pin a versioned
release for reproducible installations.

For a versioned release, update `version` in `Cargo.toml`, refresh `Cargo.lock`,
commit, and push a matching `vMAJOR.MINOR.PATCH` tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

All four platforms must pass before publishing. The tag must match `mm --version`.
Tagged releases are not overwritten; they become the latest stable release.
Archives include the binary, README, and MIT license, with individual SHA-256
files and a combined `SHA256SUMS`.

## License

MIT. Extracted from the Markdown E2E runner in
[Rally](https://github.com/jsturtevant/rally).
