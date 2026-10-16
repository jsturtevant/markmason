# Send input and pipe commands

Use a `stdin` block for input, or pipe one command's stdout into another.
These examples use the [test CLI](../support/README.md), `mm-test-cli`.

## `mm-test-cli cat`

The block is sent to stdin with a final newline.

```stdin
hello
from stdin
```

```expected
hello
from stdin
```

## `mm-test-cli echo "hello | world" | mm-test-cli cat | mm-test-cli cat`

Pipes connect stdout between stages. The quoted `|` stays inside its argument.

```expected
hello | world
```

## `mm-test-cli args "hello | world" "" "last value"`

Quotes preserve spaces and empty arguments. Commands run without a shell.

```expected
["hello | world", "", "last value"]
```
