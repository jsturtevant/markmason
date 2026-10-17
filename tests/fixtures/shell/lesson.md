---
shell: bash
setup: mm sharedset.md
---

# Inspect shared state one command at a time

## Expand a value prepared by setup

```markmason
VALUE=$(printf 'ready')
printf '%s\n' "$VALUE"
describe_session
```

```expected
ready
shared setup is ready
```

## Keep a loop together

```markmason
for word in one two; do
  printf '%s\n' "$word"
done
```

```expected
one
two
```
