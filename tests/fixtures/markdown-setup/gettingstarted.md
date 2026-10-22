# Prepare the tutorial

This file also runs on its own. When referenced by `setup: mm gettingstarted.md`,
its commands and assertions run automatically in the caller's shared fixture.

## Create application and configuration state

```markmason
mm-test-cli write-state E2E_HOME ready
mm-test-cli write-state XDG_CONFIG_HOME configured
```

## Check the initial state

```markmason
mm-test-cli state
```

```expected
ready
```
