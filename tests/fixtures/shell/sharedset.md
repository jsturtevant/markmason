---
shell: bash
---

# Prepare a shared shell environment

This prerequisite only prepares shell state. Its caller keeps the same Bash
process, so no intermediate environment file or wrapper script is necessary.

```markmason
export LESSON="persistent Bash"
STATUS=ready
describe_session() {
  printf 'shared setup is %s\n' "$STATUS"
}
```
