# Answer terminal prompts

A `pty` block waits for each prompt from the [test CLI](../support/README.md).
Sends do not add a newline; use `{enter}` to submit an answer.

## Answer repeated prompts

```markmason
mm-test-cli interactive
```

Match visible text without ANSI color codes. Repeated prompts are matched in order.

```pty
match: Ready?
send: Ada{space}Lovelace{enter}
match: Ready?
send: ready{enter}
```

```expected-contains
accepted Ada Lovelace
done ready
```

## Wait for raw terminal sequences

```markmason
mm-test-cli interactive
```

Use `match-raw` for terminal escape sequences. An empty send waits without typing.

```pty
match-raw: {hide-cursor}
send:
match: Ready?
send: yes{enter}
match: Ready?
send: no{enter}
```

```expected-contains
accepted yes
done no
```

See [runtime errors](runtime-errors.md) for missing prompts and timeouts.
