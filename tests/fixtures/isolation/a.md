---
setup: mm-test-cli
---

# First file

Setup creates home state; the second command also writes configuration state.

## Read home state created by setup

```markmason
mm-test-cli state
```

```expected
ready
```

## Write configuration state

```markmason
mm-test-cli write-state XDG_CONFIG_HOME ready
```
