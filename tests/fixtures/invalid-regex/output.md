# Invalid regex

An unclosed character class is a spec error, not a literal substring.

```markmason
mm-test-cli echo 'must not run'
```

```expected-regex
[
```
