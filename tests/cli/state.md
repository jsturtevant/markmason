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

```expected-contains
RUN  a.md
  RUN  Block 1/2 — Read home state created by setup
  ✓ PASS Block 1/2 — Read home state created by setup
  RUN  Block 2/2 — Write configuration state
  ✓ PASS Block 2/2 — Write configuration state
RUN  b.md
  RUN  Block 1/2 — Check for leaked home state
  ✓ PASS Block 1/2 — Check for leaked home state
  RUN  Block 2/2 — Check for leaked configuration state
  ✓ PASS Block 2/2 — Check for leaked configuration state
Blocks: 4 passed, 0 failed (includes deferred)
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

```expected-contains
RUN  walkthrough.md
  RUN  Block 1/2 — Inspect the prepared application state
  ✓ PASS Block 1/2 — Inspect the prepared application state
  RUN  Block 2/2 — Inspect the prepared configuration
  ✓ PASS Block 2/2 — Inspect the prepared configuration
Blocks: 2 passed, 0 failed (includes deferred)
```
