# Group related commands

Each `markmason` block is one test. Put commands on separate lines to run them
in order and check their combined output.

## Combine output from several commands

Pipelines work on individual lines. Blank lines separate commands visually
without starting another test.

```markmason
mm-test-cli echo first

mm-test-cli echo second | mm-test-cli cat
```

```expected
first
second
```

## Share file state between commands

The next command can read files written by an earlier command. Commands run
directly, so shell variables and directory changes do not carry over.

```markmason
mm-test-cli write-state TEST_HOME ready
mm-test-cli state
```

```expected
ready
```

See [error cases](errors.md) for stopping on failure, or [stdin and pipes](input.md)
for supplying input. Use an explicit shell command when you need shell scripting.
