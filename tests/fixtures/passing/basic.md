---
setup: mm --version
tags: [smoke, cli]
---
# Passing leaf commands

## Show help

```markmason
mm --help
```

```expected-contains
Standalone Markdown-driven CLI end-to-end test runner
-V, --version
Print version
```

## Show the installed version

```markmason
mm --version
```
