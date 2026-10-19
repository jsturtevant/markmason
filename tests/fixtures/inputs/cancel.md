---
inputs:
  MARKMASON_CANCEL_INPUT: Enter a value before setup
setup: mm-test-cli write-state MARKMASON_CANCEL_HOME setup-must-not-run
---

```markmason
mm-test-cli echo command-must-not-run
```
