set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

default: check

# Format the Rust source.
fmt:
    cargo fmt

# Check source formatting without modifying files.
fmt-check:
    cargo fmt --check

# Lint the binary and its regression tests.
lint:
    cargo clippy --locked --all-targets -- -D warnings

# Run Rust regression tests and the Markdown self-test suite.
test: test-rust test-markdown

# Run parser, matching, and standalone process tests.
test-rust:
    cargo test --locked

# Build the test-only CLI shared by the Markdown and Rust tests.
build-test-cli:
    cargo build --quiet --locked --example mm-test-cli

# Test the current binary against Markdown specs with bounded leaf fixtures.
test-markdown: build-test-cli
    # Inherited PWD can contain a Windows drive-letter colon; use Bash's physical path.
    root="$(pwd -P)" && PATH="$root/target/debug:$root/target/debug/examples:$PATH" cargo run --quiet --locked -- tests/cli --home-env TEST_HOME

# Run the same checks as CI.
check: fmt-check lint test

# Build a release binary for a Rust target triple.
build target:
    cargo build --release --locked --target "$1"

# Check the binary and package it with its license and checksum.
package target version: (build target)
    bash scripts/package.sh "$1" "$2"

# Publish CI artifacts as a rolling preview or immutable tagged release.
release version revision:
    bash scripts/release.sh "$1" "$2"
