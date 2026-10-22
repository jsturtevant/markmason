# Test walkthroughs in a real terminal

These tests run `mm --step` against a small [sample tutorial](../fixtures/walkthrough/lesson.md).
The `pty` blocks wait for the real prompts and send the same keys a person would
press. Run from the repository root with `mm` and `mm-test-cli` on `PATH`.

The [walkthrough guide](walkthrough.md) is tutorial input; this file tests the
interactive runner itself.

The document line shows total wall time for the selected Markdown file, including
time spent paused for inspection. The block line keeps a separate per-block timer.
These dynamic values are intentionally not hardcoded in the assertions below.

## Continue after every command, then finish

````markdown markmason
```markmason
mm tests/fixtures/walkthrough --step --file-pattern '^lesson\.md$'
```

The first two commands share a block. Enter advances from the first command to
the second, then to the next block. The final block reads its own `stdin`;
tutorial controls never become application input.

```pty
match: Enter: Next command
send: {enter}
match: Enter: Next block
send: {enter}
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
COMMAND 1 OF 2
> [running] mm-test-cli echo first
EXPECTED OUTPUT (whole block; exact, exit 0)
| first
| second
ACTUAL OUTPUT SO FAR
| first
IN PROGRESS Assertion checked after the block finishes.
COMMAND 2 OF 2
> [running] mm-test-cli echo second
ACTUAL OUTPUT
| first
| second
✓ PASS Block matched expectations.
| application input
ACTUAL OUTPUT
| application input
Walkthrough finished
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Keep a real Bash session alive between pauses

````markdown markmason
```markmason
mm tests/fixtures/shell/lesson.md --step
```

Shared Markdown setup runs without pausing. Assignments and substitutions are
real commands, so the first block pauses three times. The multiline loop is one
logical command, not one pause per physical line.

```pty
match: Enter: Next command
send: {enter}
match: Enter: Next command
send: {enter}
match: Enter: Next block
send: {enter}
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
COMMAND 1 OF 3
COMMAND 2 OF 3
| ready
COMMAND 3 OF 3
| shared setup is ready
COMMAND 1 OF 1
| one
| two
Walkthrough finished
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Run to the next block, then finish the file

The terminal redraws the same Markdown block instead of appending a new command
card. `b` runs the remaining commands and checks their assertion, then pauses
before any command in the next block. `f` runs the rest of the file.

````markdown markmason
```markmason
mm tests/fixtures/walkthrough/navigation.md --step
```

```pty
match-raw: {alt-screen}
send:
match: Enter: Next command
send: b{enter}
match: Enter: Run first command
send: f{enter}
```

```expected-contains
> [running] mm-test-cli echo first
> [running] mm-test-cli echo second
ACTUAL OUTPUT
| first
| second
✓ PASS Block matched expectations.
> [next] mm-test-cli echo third
ACTUAL OUTPUT SO FAR
| (empty)
Ready. No commands in this block have run.
| last
Walkthrough finished
Blocks: 3 passed, 0 failed (includes deferred)
```
````

## Scroll complete output without executing commands

End scrolls to the collected output; Home returns to the heading and description.
Neither key needs Enter or executes the next command.

````markdown markmason
```markmason
mm tests/fixtures/walkthrough/scrolling.md --step
```

```pty
match: Enter: Next command
send: {enter}
match: COMMAND 3 OF 3
send: {escape}[F
match: | output-30
send: {escape}[H
match: Keep the command context visible
send: {enter}
match: Enter: Finish walkthrough
send: {escape}[F
match: | finished
send: {enter}
```

```expected-contains
Keep the command context visible, and scroll to inspect every output line.
[done] mm-test-cli echo started
> [next] mm-test-cli echo finished
| output-30
| finished
✓ PASS Block matched expectations.
Walkthrough finished
Blocks: 1 passed, 0 failed (includes deferred)
```
````

## Cancel while running to the next block

Ctrl+C during command execution must unwind normally, restoring the terminal and
returning exit 130 on every supported platform.

````markdown markmason
```markmason (exit 130)
mm tests/fixtures/walkthrough/interrupt.md --step
```

```pty
match: Enter: Next command
send: b{enter}
match: > [running] mm-test-cli sleep
send: {ctrl-c}
```
````

## Cancel while finishing the file

````markdown markmason
```markmason (exit 130)
mm tests/fixtures/walkthrough/interrupt.md --step
```

```pty
match: Enter: Next command
send: f{enter}
match: > [running] mm-test-cli sleep
send: {ctrl-c}
```
````

## Honor cancellation queued with a run-ahead key

````markdown markmason
```markmason (exit 130)
mm tests/fixtures/walkthrough/interrupt.md --step
```

```pty
match: Enter: Next command
send: f{enter}{ctrl-c}
```
````

## Quit before the next command runs

````markdown markmason
```markmason
mm tests/fixtures/walkthrough --step --file-pattern '^lesson\.md$'
```

Quit at the first pause. An unfinished block is not counted as passed.

```pty
match: Enter: Next command
send: q{enter}
```

```expected-contains
COMMAND 1 OF 2
ACTUAL OUTPUT SO FAR
| first
Walkthrough stopped
Blocks: 0 passed, 0 failed (includes deferred)
```
````

## Search for a file in the picker

````markdown markmason
```markmason
mm tests/fixtures/walkthrough --step
```

Without a file filter, choose the lesson by typing its name. The default choice is
`a-other.md`, so the lesson only runs if the search selects it.

```pty
match: a-other.md
send: lesson{enter}
match: Enter: Next command
send: {enter}
match: Enter: Next block
send: {enter}
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
MARKMASON WALKTHROUGH
COMMAND 1 OF 2
COMMAND 2 OF 2
Walkthrough finished
Blocks: 2 passed, 0 failed (includes deferred)
```
````

## Cancel the picker without running a tutorial

````markdown markmason
```markmason
mm tests/fixtures/walkthrough --step
```

```pty
match: a-other.md
send: {escape}
```

```expected-contains
Walkthrough cancelled. No commands were run.
```
````

## Inspect an assertion failure before exiting

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/walkthrough --step --file-pattern '^mismatch\.md$'
```

A failed assertion still pauses for inspection and keeps a nonzero exit status.

```pty
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
EXPECTED OUTPUT (exact, exit 0)
| different
ACTUAL OUTPUT
| actual
✗ FAIL
| Line 1 mismatch:
Walkthrough finished
Blocks: 0 passed, 1 failed (includes deferred)
```
````

## Select a file explicitly when no terminal is available

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/walkthrough --step
```

Without `pty`, the picker reports how to select a file instead of blocking on
terminal input.

```expected
Error: The walkthrough file picker needs an interactive terminal; select a file with --file-pattern REGEX
```
````

## Prepare a walkthrough with another Markdown document

````markdown markmason
```markmason
mm tests/fixtures/markdown-setup/walkthrough.md --step
```

A direct file path skips the picker. The walkthrough's
`setup: mm gettingstarted.md` runs automatically in the same fixture, without
pausing. Only the two walkthrough commands need Enter.

```pty
match: Enter: Next block
send: {enter}
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
ACTUAL OUTPUT
| ready
✓ PASS Block matched expectations.
ACTUAL OUTPUT
| true
✓ PASS Block matched expectations.
Walkthrough finished
Blocks: 2 passed, 0 failed (includes deferred)
```
````
