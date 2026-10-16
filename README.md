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
cargo install --git https://github.com/jsturtevant/markmason --tag v0.1.1 --locked
```

## Write a test

Save as `tests/cli/greet.md`:

````markdown
# Greetings

## `bash -c "echo Hello, world!"`
```expected
Hello, world!
```
````

With `bash` on `PATH`, run:

```sh
mm                         # Defaults to tests/cli
mm path/to/specs            # Another directory
mm --list                  # Discover without executing
mm --file-pattern greet    # Select files by regex
```

## Live examples

Learn more with real examples from `mm`:

- [Getting started](tests/cli/cli.md)
- [Running tests](tests/cli/running.md)
- [Filtering](tests/cli/filtering.md)
- [Stdin and pipes](tests/cli/input.md)
- [Terminal prompts](tests/cli/interactive.md)
- [Output matching](tests/cli/matching.md)
- [Setup and state](tests/cli/state.md)
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
| Exact output | `expected` block |
| Ordered whole-line matching | `expected-contains` block; no unmatched trailing lines |
| Expected failure | ``## `command` (exit 1)`` |
| Piped input | `stdin` block |
| Interactive terminal | `pty` block with `match:` / `send:` |
| Per-file setup | `setup: python3 setup.py` in YAML frontmatter |
| Clone a fixture | `clone: owner/repo` in frontmatter; requires authenticated `gh` |
| Tags and timeout | `tags: [smoke]`, `timeout: 30` in frontmatter |

Commands resolve from `PATH`, without a shell. Each file gets an isolated home
and configuration directory; its cases run in order and share state.

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
- For stable releases, update `Cargo.toml`, `Cargo.lock`, and the version assertion
  in `tests/cli/cli.md`, then push a matching `vMAJOR.MINOR.PATCH` tag.
  Versioned releases are never overwritten.
- All four platforms must pass before publishing. Archives include the binary,
  README, MIT license, and SHA-256 checksums.

Package locally with `just package TARGET preview-release` (7-Zip needed on Windows).

## License

[MIT](LICENSE). Extracted from [Rally](https://github.com/jsturtevant/rally).
