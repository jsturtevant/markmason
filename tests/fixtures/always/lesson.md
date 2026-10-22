---
shell: bash
---

# Clean up example state

Use a deferred block to remove state after the ordinary commands finish.

## Remove the state at the end

Although this block appears first, it runs last. Keep cleanup safe when earlier
commands might not have created the state yet.

```bash markmason always
cleanup_example
```

```expected
cleaned example state
```

## Define cleanup and create state

The function and later blocks share one Bash session.

```markmason
cleanup_example() {
  rm -f "$E2E_HOME/state"
  printf 'cleaned example state\n'
}
mm-test-cli write-state E2E_HOME ready
```

## Read the state before cleanup

```markmason
mm-test-cli state
```

```expected
ready
```
