---
tags: [tutorial]
---

# Sample tutorial

## Build output one command at a time

```markmason
mm-test-cli echo first
mm-test-cli echo second
```

```expected
first
second
```

## Send input to the application

```markmason
mm-test-cli cat
```

```stdin
application input
```

```expected
application input
```
