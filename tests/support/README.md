# The test CLI

`mm` runs the tests. `mm-test-cli` is a small app for those tests to run.
It prints predictable output, reads stdin, shows prompts, and writes state.
It can also fail or wait too long on purpose.

This app is only for testing Markmason. It is not included in releases, and
you do not need it in another project: your specs run your own CLI instead.

## Why some sample suites fail

The [sample suites](../fixtures/README.md) are inputs to `mm`.
Some should pass; others are deliberately broken.
Tests in `tests/cli` check their output and exit codes: a broken suite should
fail, and the outer test passes if `mm` reports the right error.

## Run it

From the repository root:

```sh
just build-test-cli
cargo run --quiet --locked --example mm-test-cli -- echo hello
```

The second command prints `hello`. To run the full set of executable examples:

```sh
just test-markdown
```

That builds both programs, puts them on `PATH`, and runs `tests/cli`.
The runner creates isolated directories with `--home-env TEST_HOME`;
state commands use that environment rather than your real home.

## Commands used in the guides

These are `mm-test-cli` commands, not options or subcommands of `mm`.

| Command | What it does |
|---------|--------------|
| `echo ...`, `lines ...`, `args ...` | Print arguments as text, separate lines, or a quoted list |
| `cat` | Read stdin and print it back |
| `interactive` | Print two colored `Ready?` prompts and read an answer to each |
| `env NAME ...`, `cwd`, `project` | Print environment values, the working directory, or its last path component |
| No arguments | Write `ready` to `$TEST_HOME/state`; used by per-file setup |
| `setup FILE` | Check that the setup file exists and write `ready` to `$TEST_HOME/state` |
| `state`, `mark PATH`, `write-state NAME VALUE`, `state-exists [NAME]` | Read, change, or check state in the test's directories |
| `warn`, `fail` | Print to stdout and stderr; `fail` also exits with code 2 |
| `sleep`, `premature` | Wait ten seconds, or print `done` and exit before a scripted prompt |
| `with-env NAME VALUE ... -- COMMAND ...` | Set variables for one child command and return its exit code |

In `write-state` and `state-exists`, `NAME` is a directory environment variable,
such as `XDG_CONFIG_HOME`. `state-exists` defaults to `TEST_HOME`.

See the [input](../cli/input.md), [terminal](../cli/interactive.md),
[state](../cli/state.md), and [environment](../cli/environment.md) guides
for examples with assertions.

## Shared with Rust tests

The [Rust regression tests](../runner.rs) compile the same [source](test_cli.rs)
as `app`. Extra commands there exercise process cleanup and fake GitHub cloning.
Keeping one test app gives both suites the same behavior without another runtime.
