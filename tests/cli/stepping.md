# Test walkthroughs in a real terminal

These tests run `mm --step` against a small [sample tutorial](../fixtures/walkthrough/lesson.md).
The `pty` blocks wait for the real prompts and send the same keys a person would
press. Run from the repository root with `mm` and `mm-test-cli` on `PATH`.

The [walkthrough guide](walkthrough.md) is tutorial input; this file tests the
interactive runner itself.

## Continue after every command, then finish

```markmason
mm tests/fixtures/walkthrough --step --file-pattern '^lesson\.md$' --home-env TEST_HOME
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
lesson.md | Block 1 of 2
Build output one command at a time
COMMAND 1 OF 2
$ mm-test-cli echo first
EXPECTED OUTPUT (whole block; exact, exit 0)
| first
| second
ACTUAL OUTPUT SO FAR
| first
IN PROGRESS Assertion checked after the block finishes.
COMMAND 2 OF 2
$ mm-test-cli echo second
ACTUAL OUTPUT
| first
| second
PASS Block matched expectations.
lesson.md | Block 2 of 2
Send input to the application
| application input
ACTUAL OUTPUT
| application input
Walkthrough finished
Passed blocks: 2
Failed blocks: 0
```

## Keep a real Bash session alive between pauses

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
lesson.md | Block 1 of 2
COMMAND 1 OF 3
COMMAND 2 OF 3
| ready
COMMAND 3 OF 3
| shared setup is ready
lesson.md | Block 2 of 2
COMMAND 1 OF 1
| one
| two
Walkthrough finished
Passed blocks: 2
Failed blocks: 0
```

## Quit before the next command runs

```markmason
mm tests/fixtures/walkthrough --step --file-pattern '^lesson\.md$' --home-env TEST_HOME
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
Passed blocks: 0
Failed blocks: 0
```

## Search for a file in the picker

```markmason
mm tests/fixtures/walkthrough --step --home-env TEST_HOME
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
lesson.md | Block 1 of 2
COMMAND 1 OF 2
COMMAND 2 OF 2
lesson.md | Block 2 of 2
Walkthrough finished
Passed blocks: 2
Failed blocks: 0
```

## Cancel the picker without running a tutorial

```markmason
mm tests/fixtures/walkthrough --step --home-env TEST_HOME
```

```pty
match: a-other.md
send: {escape}
```

```expected-contains
Walkthrough cancelled. No commands were run.
```

## Inspect an assertion failure before exiting

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
FAIL
| Line 1 mismatch:
Walkthrough finished
Passed blocks: 0
Failed blocks: 1
```

## Select a file explicitly when no terminal is available

```markmason (exit 1)
mm tests/fixtures/walkthrough --step
```

Without `pty`, the picker reports how to select a file instead of blocking on
terminal input.

```expected
Error: The walkthrough file picker needs an interactive terminal; select a file with --file-pattern REGEX
```

## Prepare a walkthrough with another Markdown document

```markmason
mm tests/fixtures/markdown-setup/walkthrough.md --step --home-env TEST_HOME
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
walkthrough.md | Block 1 of 2
Inspect the prepared application state
ACTUAL OUTPUT
| ready
PASS Block matched expectations.
walkthrough.md | Block 2 of 2
Inspect the prepared configuration
ACTUAL OUTPUT
| true
PASS Block matched expectations.
Walkthrough finished
Passed blocks: 2
Failed blocks: 0
```
