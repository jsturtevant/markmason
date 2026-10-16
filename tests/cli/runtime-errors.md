# Timeouts and unfinished prompts

These examples deliberately time out or miss terminal prompts.
An execution error still fails a case that expects a nonzero exit.

## `mm tests/fixtures/command-timeout --timeout 1` (exit 1)

[Expecting exit 1](../fixtures/command-timeout/timeout.md) cannot turn a timeout
into a pass. The limit is in seconds.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli sleep: Command timed out after 1000ms: mm-test-cli sleep
Output so far:
before timeout
```

## `mm tests/fixtures/file-timeout --timeout 5` (exit 1)

[Frontmatter](../fixtures/file-timeout/timeout.md) sets a one-second timeout,
overriding the five-second CLI default.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli sleep: Command timed out after 1000ms: mm-test-cli sleep
Output so far:
before timeout
```

## `mm tests/fixtures/pty-timeout` (exit 1)

[A terminal timeout](../fixtures/pty-timeout/timeout.md) reports the prompt it was
waiting for, along with output captured so far.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli sleep: PTY command timed out after 1000ms.
Waiting for step 0: match "missing"
Output so far:
before timeout
```

## `mm tests/fixtures/pty-incomplete` (exit 1)

[Exiting successfully](../fixtures/pty-incomplete/prompt.md) is not enough:
every terminal step must finish.

```expected-contains
pass 0
fail 1
FAIL mm-test-cli premature: PTY exited before completing step 0: match "missing"
Output:
done
```
