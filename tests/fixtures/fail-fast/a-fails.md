# Failing document

The assertion fails, but deferred cleanup still runs before the suite stops.

## Produce the wrong value

```markmason
mm-test-cli echo actual
```

```expected
expected
```

## Finish cleanup

```markmason always
mm-test-cli echo cleaned
```

```expected
cleaned
```
