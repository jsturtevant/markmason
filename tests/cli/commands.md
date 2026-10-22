# Group related commands

Each `markmason` block is one test. Put commands on separate lines to run them
in order and check their combined output.

## Use the direct form by default

**This is the recommended form for normal executable documentation.** GitHub
renders the command and expected output without showing the Markmason fence
syntax, keeping the guide focused on the CLI being taught.

```markmason
mm-test-cli echo direct
```

```expected
direct
```

## Combine output from several commands

Pipelines work on individual lines. Blank lines separate commands visually
without starting another test. The outer `markdown markmason` fence makes the
inner executable Markdown visible when this page is rendered. Use that wrapper
only when documenting Markmason itself and showing its syntax is the point.

````markdown markmason
```markmason
mm-test-cli echo first

mm-test-cli echo second | mm-test-cli cat
```

```expected
first
second
```
````

## Share file state between commands

The next command can read files written by an earlier command. Commands run
directly, so shell variables and directory changes do not carry over.

````markdown markmason
```markmason
mm-test-cli write-state E2E_HOME ready
mm-test-cli state
```

```expected
ready
```
````

See [error cases](errors.md) for stopping on failure, or [stdin and pipes](input.md)
for supplying input. Use an explicit shell command when you need shell scripting.
