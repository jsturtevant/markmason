# Substring mismatch

This deliberately fails: contains matching still requires a whole line.

## Print a line that only partially matches

```markmason
mm-test-cli lines "prefix first"
```

```expected-contains
first
```
