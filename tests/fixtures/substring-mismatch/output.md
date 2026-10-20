# Missing literal phrase

This intentionally expects a phrase that is not in the output.

```markmason
mm-test-cli lines 'prefix first' 'second trailing'
```

```expected-substring
missing phrase
```
