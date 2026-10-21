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
cargo install --git https://github.com/jsturtevant/markmason --tag v0.4.1 --locked
```

## Write a test

Save as `tests/cli/version.md`:

````markdown
# Check Markmason

## Show the installed version

```markmason
mm --version
```

```expected
mm 0.4.1
```
````

To use Bash explicitly:

````markdown
## Say hello with Bash

```markmason
bash -c "echo Hello, world!"
```

```expected
Hello, world!
```
````

Run Markmason:

```sh
mm                         # Defaults to tests/cli
mm path/to/specs            # Another directory
mm path/to/walkthrough.md   # One Markdown file
mm --list                  # Discover without executing
mm --fail-fast             # Stop after the first failed document cleans up
mm --parallel              # Use available logical CPUs for independent files
mm --parallel 8            # Bound file execution to eight workers
mm --file-pattern greet    # Select files by regex
mm --step                  # Pick a file and walk through it
mm --step --file-pattern '^greet\.md$'  # Skip the picker
mm path/to/walkthrough.md --step       # Walk through one file directly
```

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
- [Error cases](tests/cli/errors.md)
- [Matching failures](tests/cli/matching-errors.md)
- [Runtime failures and timeouts](tests/cli/runtime-errors.md)
- [Setup and state](tests/cli/state.md)
- [Required inputs](tests/cli/inputs.md)
- [Repository fixtures](tests/cli/fixtures.md)
- [Persistent Bash sessions](tests/cli/shell.md)
- [Deferred cleanup](tests/cli/cleanup.md)
- [Path variables](tests/cli/variables.md)
- [Environment filters](tests/cli/environment.md)

Run the same files from this checkout (see [development prerequisites](#develop-and-self-test)):

```sh
just test-markdown
```

This builds `mm` and [`mm-test-cli`](tests/support/README.md), a small app
that prints text, shows prompts, and writes test state for demonstrating Markmason features.

The [specification reference](docs/specification.md) may help users who need exact
behavior, but is primarily written for agents and contributors developing
Markmason itself. Use `mm --help` for CLI options.

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
