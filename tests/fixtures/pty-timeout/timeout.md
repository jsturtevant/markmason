---
timeout: 1
---

# Terminal timeout

The process stays alive without printing the prompt the script needs.

## Wait for a prompt that never arrives

```markmason
mm-test-cli sleep
```

```pty
match: missing
send: yes{enter}
```
