# Second file

The previous file's home and configuration state must not leak into this file.

## Check for leaked home state

```markmason
mm-test-cli state-exists
```

```expected
false
```

## Check for leaked configuration state

```markmason
mm-test-cli state-exists XDG_CONFIG_HOME
```

```expected
false
```
