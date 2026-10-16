---
setup: mm-test-cli setup ../fixtures/setup.fixture
---

# Set up and share state

Setup runs once per file; its cases then run in order and share a temporary home.
The [test CLI](../support/README.md) uses `TEST_HOME`, set by `just test-markdown`.

## `mm-test-cli state`

Setup found its argument relative to this spec and wrote the initial state.

```expected
ready
```

## `mm-test-cli mark "$TEST_HOME/state"`

Overwrite that state using the runner-provided home variable.

## `mm-test-cli state`

The next command sees the change. Run this file as a whole, not just this case.

```expected
unexpected
```

## `mm tests/fixtures/isolation --home-env TEST_HOME`

[These two files](../fixtures/isolation) get separate homes.
The second cannot see the first file's home or configuration state.

```expected
a.md
PASS mm-test-cli state
PASS mm-test-cli write-state XDG_CONFIG_HOME ready
b.md
PASS mm-test-cli state-exists
PASS mm-test-cli state-exists XDG_CONFIG_HOME
pass 4
fail 0
```
