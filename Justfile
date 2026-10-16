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

# Run parser, matching, and standalone process tests.
test:
    cargo test --locked

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
