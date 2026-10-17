---
setup: mm-test-cli setup ../fixtures/setup.fixture
---

# Set up and share state

Setup runs once per file; its cases then run in order and share a temporary home.
The [test CLI](../support/README.md) uses `TEST_HOME`, set by `just test-markdown`.

## Read the state created by setup

```markmason
mm-test-cli state
```

Setup found its argument relative to this spec and wrote the initial state.

```expected
ready
```

## Change shared state

```markmason
mm-test-cli mark "$TEST_HOME/state"
```

Overwrite that state using the runner-provided home variable.

## Read the changed state

```markmason
mm-test-cli state
```

The next command sees the change. Run this file as a whole, not just this case.

```expected
unexpected
```

## Keep state isolated between files

```markmason
mm tests/fixtures/isolation --home-env TEST_HOME
```

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

## Reuse a getting-started document

Put `setup: mm gettingstarted.md` in a walkthrough's frontmatter to run that
document's commands and assertions first. Paths resolve beside the calling
Markdown file. Setup shares the walkthrough's home, configuration, and working
directory, so its state remains available until the walkthrough finishes.

The [getting-started document](../fixtures/markdown-setup/gettingstarted.md)
prepares state for [this walkthrough](../fixtures/markdown-setup/walkthrough.md).
Both files can also run directly:

```markmason
mm tests/fixtures/markdown-setup/walkthrough.md --home-env TEST_HOME
```

```expected
walkthrough.md
PASS mm-test-cli state
PASS mm-test-cli state-exists XDG_CONFIG_HOME
pass 2
fail 0
```
