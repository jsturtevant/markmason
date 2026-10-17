---
setup: mm gettingstarted.md
---

# Continue after getting started

Setup runs automatically before the first walkthrough command.

## Inspect the prepared application state

```markmason
mm-test-cli state
```

```expected
ready
```

## Inspect the prepared configuration

```markmason
mm-test-cli state-exists XDG_CONFIG_HOME
```

```expected
true
```
