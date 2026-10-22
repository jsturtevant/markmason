# Timeouts and unfinished prompts

These examples deliberately time out or miss terminal prompts.
An execution error still fails a case that expects a nonzero exit.

## Report a missing executable

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/missing-executable
```

A command that cannot be found explains that it is missing from `PATH` and how
to make it available.

```expected-substring
  ✗ FAIL  Block 1/1 — Start a command that is not on PATH
    Document: missing.md
    Command: missing-executable-markmason
    Failed to start "missing-executable-markmason": executable not found on PATH. Install or build it, then add its directory to PATH
```
````

## Fail on timeout despite an expected error exit

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/command-timeout --timeout 1
```

[Expecting exit 1](../fixtures/command-timeout/timeout.md) cannot turn a timeout
into a pass. The limit is in seconds.

```expected-substring
  ✗ FAIL  Block 1/1 — Exceed the command timeout
    Document: timeout.md
    Command: mm-test-cli sleep
    Command timed out after 1000ms: mm-test-cli sleep
    Output so far:
    before timeout
```
````

## Use the per-file timeout

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/file-timeout --timeout 5
```

[Frontmatter](../fixtures/file-timeout/timeout.md) sets a one-second timeout,
overriding the five-second CLI default.

```expected-substring
  ✗ FAIL  Block 1/1 — Exceed the per-file timeout
    Document: timeout.md
    Command: mm-test-cli sleep
    Command timed out after 1000ms: mm-test-cli sleep
    Output so far:
    before timeout
```
````

## Report a missing terminal prompt

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/pty-timeout
```

[A terminal timeout](../fixtures/pty-timeout/timeout.md) reports the prompt it was
waiting for, along with output captured so far.

```expected-substring
  ✗ FAIL  Block 1/1 — Wait for a prompt that never arrives
    Document: timeout.md
    Command: mm-test-cli sleep
    PTY command timed out after 1000ms.
    Waiting for step 0: match "missing"
    Output so far:
    before timeout
```
````

## Require every terminal step to finish

````markdown markmason
```markmason (exit 1)
mm tests/fixtures/pty-incomplete
```

[Exiting successfully](../fixtures/pty-incomplete/prompt.md) is not enough:
every terminal step must finish.

```expected-substring
  ✗ FAIL  Block 1/1 — Exit before the requested prompt
    Document: prompt.md
    Command: mm-test-cli premature
    PTY exited before completing step 0: match "missing"
    Output:
    done
```
````
