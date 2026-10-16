# Second file

The previous file's home and configuration state must not leak into this file.

## `mm-test-cli state-exists`

```expected
false
```

## `mm-test-cli state-exists XDG_CONFIG_HOME`

```expected
false
```
