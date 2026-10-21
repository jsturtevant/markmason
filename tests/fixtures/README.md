# Sample suites

These directories contain small Markdown suites used to test `mm` itself.
Some should pass; others are deliberately broken.

The [outer tests](../cli) run `mm` on these inputs and check its output and exit
code. Many inputs use [mm-test-cli](../support/README.md), a small app that supplies
predictable output, prompts, and state.

## Passing and broken inputs

| Example | Expected result | What it checks |
|---------|-----------------|----------------|
| [passing](passing) | Exit 0 | Valid specs run and report the right totals |
| [always](always/lesson.md) | Exit 0 | Deferred cleanup shares Bash state and runs after ordinary commands |
| [fail-fast](fail-fast) | Exit 1 with `--fail-fast` | Cleanup finishes and later documents are reported as not run |
| [parallel](parallel) | Exit 0 with `--parallel 2` | File output stays ordered and `parallel: false` acts as a barrier |
| [matching-order](matching-order/output.md) | Exit 1 | Output in the wrong order is rejected |
| [substring-mismatch](substring-mismatch/output.md) | Exit 1 | Missing literal text reports the substring and actual output |
| [regex-mismatch](regex-mismatch/output.md) | Exit 1 | A missing regex match reports the pattern and actual output |
| [invalid-regex](invalid-regex/output.md) | Exit 1 | Malformed regexes fail before running commands |
| [invalid](invalid/invalid.md) | Exit 1 | Invalid frontmatter is rejected |
| [pty-timeout](pty-timeout/timeout.md) | Exit 1 | An unreached terminal prompt times out |
| [walkthrough](walkthrough) | Mixed | Terminal-driven stepping, file selection, quitting, and assertion failures |
| [shell](shell/lesson.md) | Exit 0 | Persistent Bash state, shared Markdown setup, and logical-command stepping |

For example, `matching-order` prints `second`, then `first`, but expects `first`,
then `second`. The [outer test](../cli/matching-errors.md) expects exit 1 and the
message `Expected line 2 not found in output`.

That outer test **passes when `mm` correctly reports the failure**.
It fails if `mm` accepts the broken input or reports a different error.

## Run them

From the repository root, run the outer tests:

```sh
just test-markdown
```

This builds `mm` and `mm-test-cli`. To inspect a sample suite directly afterward,
put those builds on your shell's `PATH`:

```sh
export PATH="$(pwd -P)/target/debug:$(pwd -P)/target/debug/examples:$PATH"
mm tests/fixtures/passing
```

To see a deliberate failure:

```sh
mm tests/fixtures/matching-order
```

That command should exit with code 1. Run a named subdirectory, not this whole
directory: it groups separate scenarios and includes this non-executable README.
Some suites need flags such as `--home-env`; their outer tests show the full command.
