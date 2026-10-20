# Timeouts and unfinished prompts

These examples deliberately time out or miss terminal prompts.
An execution error still fails a case that expects a nonzero exit.

## Fail on timeout despite an expected error exit

```markmason (exit 1)
mm tests/fixtures/command-timeout --timeout 1
```

[Expecting exit 1](../fixtures/command-timeout/timeout.md) cannot turn a timeout
into a pass. The limit is in seconds.

```expected-regex
(?s)✗ FAIL  Block 1/1 — Exceed the command timeout.*Command timed out after 1000ms: mm-test-cli sleep.*Output so far:.*before timeout.*Blocks: 0 passed, 1 failed
```

## Use the per-file timeout

```markmason (exit 1)
mm tests/fixtures/file-timeout --timeout 5
```

[Frontmatter](../fixtures/file-timeout/timeout.md) sets a one-second timeout,
overriding the five-second CLI default.

```expected-regex
(?s)✗ FAIL  Block 1/1 — Exceed the per-file timeout.*Command timed out after 1000ms: mm-test-cli sleep.*Output so far:.*before timeout.*Blocks: 0 passed, 1 failed
```

## Report a missing terminal prompt

```markmason (exit 1)
mm tests/fixtures/pty-timeout
```

[A terminal timeout](../fixtures/pty-timeout/timeout.md) reports the prompt it was
waiting for, along with output captured so far.

```expected-regex
(?s)✗ FAIL  Block 1/1 — Wait for a prompt that never arrives.*PTY command timed out after 1000ms.*Waiting for step 0: match "missing".*Output so far:.*before timeout.*Blocks: 0 passed, 1 failed
```

## Require every terminal step to finish

```markmason (exit 1)
mm tests/fixtures/pty-incomplete
```

[Exiting successfully](../fixtures/pty-incomplete/prompt.md) is not enough:
every terminal step must finish.

```expected-regex
(?s)✗ FAIL  Block 1/1 — Exit before the requested prompt.*PTY exited before completing step 0: match "missing".*Output:.*done.*Blocks: 0 passed, 1 failed
```
