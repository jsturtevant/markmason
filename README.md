# Markmason

Write tests in Markdown. Run them with **`mm`**.

## Install

Download from [Releases](https://github.com/jsturtevant/markmason/releases),
verify the archive against `SHA256SUMS`, and put `mm` (`mm.exe` on Windows) on `PATH`.

| Platform | Target |
|----------|--------|
| Linux x64, glibc 2.35+ | `x86_64-unknown-linux-gnu` |
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` |

Or install from source with Rust 1.88+:

```sh
cargo install --git https://github.com/jsturtevant/markmason --tag v0.3.0 --locked
```

## Write a test

Save as `tests/cli/greet.md`:

````markdown
# Greetings

## Say hello

```markmason
bash -c "echo Hello, world!"
```

```expected
Hello, world!
```
````

With `bash` on `PATH`, run:

```sh
mm                         # Defaults to tests/cli
mm path/to/specs            # Another directory
mm path/to/walkthrough.md   # One Markdown file
mm --list                  # Discover without executing
mm --file-pattern greet    # Select files by regex
mm --step                  # Pick a file and walk through it
mm --step --file-pattern '^greet\.md$'  # Skip the picker
mm path/to/walkthrough.md --step       # Walk through one file directly
```

With `--step`, choose a file using the arrow keys or type to search, then press
**Enter**. **Esc** cancels without running anything. An explicit `--file-pattern`
(or its environment variable) or a direct Markdown file path skips the picker.

During the walkthrough, a fullscreen Markdown-style view shows the section heading,
description, full code block, and expected/actual output. The arrow points to the
next command; completed commands retain green `[done]` prefixes.
Nothing is collapsed or truncated: long lines wrap, and **Up/Down**,
**PgUp/PgDn**, **Home/End**, or the **mouse wheel** scroll without running commands.
Keep sections short so they normally fit. Actual output accumulates within the
block and resets at the next block.
Press **Enter** for the next command, **b then Enter** to run to the next block,
**f then Enter** to finish the file, or **q then Enter** to quit. These shortcuts
run commands rather than skipping them; failures always pause for inspection.

To prepare a walkthrough from another Markdown document, add
`setup: mm gettingstarted.md` in its frontmatter. The prerequisite runs
automatically in the walkthrough's shared environment, so the state it creates
is available to later steps. See [setup and state](tests/cli/state.md).

Declare required environment values instead of supplying a long command prefix:

```yaml
inputs:
  KUBECONFIG: Path to the kubeconfig for your disposable cluster
  STORAGE_CONFIG: Name of the existing storage configuration
```

With `--step`, missing inputs are prompted for before setup; existing nonempty
environment values are reused. Unattended runs fail clearly if inputs are
missing, and `--list` never prompts. Values are plain text, not secret prompts.
Inputs declared by Markdown prerequisites are collected before any setup runs.
See the [executable input examples](tests/cli/inputs.md).

Add `shell: bash` when the tutorial needs shell variables, functions, pipelines,
or `cd` changes to persist across commands and Markdown setup:

````markdown
---
shell: bash
setup: mm sharedset.md
---

```markmason
MESSAGE="hello from Bash"
printf '%s\n' "$MESSAGE"
```
```expected
hello from Bash
```
````

This requires Bash on `PATH`: system Bash on Linux/macOS (including macOS
Bash 3.2), or Git Bash for native Windows. There is no automatic PowerShell,
cmd, or WSL substitution. See the [executable shell guide](tests/cli/shell.md).

## Live examples

Learn more with real examples from `mm`:

- [Getting started](tests/cli/cli.md)
- [Command blocks](tests/cli/commands.md)
- [Step-by-step walkthrough](tests/cli/walkthrough.md)
- [Walkthroughs tested in a real terminal](tests/cli/stepping.md)
- [Running tests](tests/cli/running.md)
- [Filtering](tests/cli/filtering.md)
- [Stdin and pipes](tests/cli/input.md)
- [Terminal prompts](tests/cli/interactive.md)
- [Output matching](tests/cli/matching.md)
- [Setup and state](tests/cli/state.md)
- [Persistent Bash sessions](tests/cli/shell.md)
- [Path variables](tests/cli/variables.md)
- [Environment filters](tests/cli/environment.md)

Run the same files from this checkout (see [development prerequisites](#develop-and-self-test)):

```sh
just test-markdown
```

This builds `mm` and [`mm-test-cli`](tests/support/README.md), a small app
that prints text, shows prompts, and writes test state. Both go on `PATH`.
`mm-test-cli` is not shipped with `mm`; your own specs run your CLI instead.

## Markmason features

| Feature | Syntax |
|---------|--------|
| Run a command | `markmason` block |
| Interactive walkthrough | `--step`, combined with the usual filters |
| Exact output | `expected` block |
| Ordered whole-line matching | `expected-contains` block; no unmatched trailing lines |
| Expected failure | `markmason (exit 1)` on the opening fence |
| Piped input | `stdin` block |
| Interactive terminal | `pty` block with `match:` / `send:` |
| Per-file setup | `setup: python3 setup.py` in YAML frontmatter |
| Shared Markdown setup | `setup: mm sharedset.md` |
| Persistent Bash session | `shell: bash` in YAML frontmatter |
| Clone a fixture | `clone: owner/repo` in frontmatter; requires authenticated `gh` |
| Tags and timeout | `tags: [smoke]`, `timeout: 30` in frontmatter |

By default, commands resolve from `PATH` without a shell. `shell: bash` opts
one file into a persistent Bash session. Each file gets an isolated home and
configuration directory; its cases run in order and share state.

Headings describe the behavior. Each `markmason` block is one test: commands run
in order, combine their output, and stop at the first nonzero exit.

See the [specification reference](docs/specification.md) for setup, matching,
PTYs, variables, filters, and cleanup details. Use `mm --help` for CLI options.

## Develop and self-test

Requires Rust with `rustfmt` and `clippy`, Bash, and
[Just](https://github.com/casey/just) (`cargo install --locked just`).

```sh
just check            # Formatting, lint, Rust tests, Markdown self-tests
just test-markdown    # Only the Markdown self-tests
just fmt              # Format Rust source
```

The [Markdown self-tests](tests/cli) run `mm` against small
[fixture specs](tests/fixtures) and the [test CLI](tests/support/README.md).
Nested fixtures call leaf commands, never the self-test suite.
No Node, GitHub authentication, or network access is needed. Cloning, cancellation,
and descendant cleanup remain covered by the Rust integration tests.

## Write tests with an agent

Use the [markmason-tests skill](.github/skills/markmason-tests/SKILL.md) to write
tests that read like docs. It covers normal examples, separate error cases,
assertions, fixtures, and safe self-testing.

Copy `.github/skills/markmason-tests/` into the same location in another repo
to reuse it. Keep the folder's reference file and license with it.

## Releases

PRs run CI only. The separate release workflow reuses those checks before publishing.

- Successful `main` builds update the mutable **`preview-release`** prerelease.
- Use a minor version bump for executable Markdown format changes.
- For stable releases, update `Cargo.toml`, `Cargo.lock`, and the version assertion
  in `tests/cli/cli.md`, then push a matching `vMAJOR.MINOR.PATCH` tag.
  Versioned releases are never overwritten.
- All four platforms must pass before publishing. Archives include the binary,
  README, MIT license, and SHA-256 checksums.

Package locally with `just package TARGET preview-release` (7-Zip needed on Windows).

## License

[MIT](LICENSE). Extracted from [Rally](https://github.com/jsturtevant/rally).
