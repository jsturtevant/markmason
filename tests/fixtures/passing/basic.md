---
setup: mm --version
tags: [smoke, cli]
---
# Passing leaf commands

## `mm --help`

```expected-contains
Usage: mm [OPTIONS] [SPEC_DIR]
-V, --version
Print version
```

## `mm --version`

## `mm --not-an-option | mm --help` (exit 2)

The pipeline must stop at the first failing command.

```expected-contains
error: unexpected argument '--not-an-option' found
Usage: mm [OPTIONS] [SPEC_DIR]
For more information, try '--help'.
```
