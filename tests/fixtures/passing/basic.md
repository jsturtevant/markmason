---
setup: mm --version
tags: [smoke, cli]
---
# Passing leaf commands

## `mm --help`

```expected-contains
Standalone Markdown-driven CLI end-to-end test runner
-V, --version
Print version
```

## `mm --version`

## `mm --not-an-option | mm --help` (exit 2)

The pipeline must stop at the first failing command.

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```
