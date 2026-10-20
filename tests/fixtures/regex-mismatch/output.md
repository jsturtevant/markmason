# Missing regex match

This intentionally expects a numeric ID but receives a word.

```markmason
mm-test-cli lines 'id: unknown' 'request complete'
```

```expected-regex
id: \d+
```
