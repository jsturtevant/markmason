# Markmason

Write CLI tests in Markdown. Run them with **`mm`**, a single Rust binary.
No Node or Rust toolchain needed at runtime.

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
cargo install --git https://github.com/jsturtevant/markmason --tag v0.1.0 --locked
```

## Write a test

Save as `tests/cli/greet.md`:

````markdown
# Greetings

## `mycli greet Ada`
```expected
Hello, Ada!
```
````

Put your CLI on `PATH`, then run:

```sh
mm                         # Defaults to tests/cli
mm path/to/specs            # Another directory
mm --list                  # Discover without executing
mm --file-pattern greet    # Select files by regex
```

## Test features

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
[fixture specs](tests/fixtures). Those fixtures only invoke leaf commands such
as `mm --help`, never the self-test suite: nesting is finite, not recursive.
No Node, GitHub authentication, or network access is needed.

## Releases

- Successful `main` builds update the mutable **`preview-release`** prerelease.
- For stable releases, update `Cargo.toml`, `Cargo.lock`, and the version assertion
  in `tests/cli/cli.md`, then push a matching `vMAJOR.MINOR.PATCH` tag.
  Versioned releases are never overwritten.
- All four platforms must pass before publishing. Archives include the binary,
  README, MIT license, and SHA-256 checksums.

Package locally with `just package TARGET preview-release` (7-Zip needed on Windows).

## License

[MIT](LICENSE). Extracted from [Rally](https://github.com/jsturtevant/rally).
