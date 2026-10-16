# Selecting files and cases

## `mm tests/fixtures/passing --file-pattern basic`

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --file-exclude basic`

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## `mm tests/fixtures/passing --tags SMOKE,cli`

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --tags-exclude smoke`

```expected
extra.md
PASS mm --version
pass 1
fail 0
```

## `mm tests/fixtures/passing --test-name-pattern basic.md`

```expected
basic.md
PASS mm --help
PASS mm --version
PASS mm --not-an-option | mm --help
pass 3
fail 0
```

## `mm tests/fixtures/passing --test-name-pattern "^mm --version$"`

```expected
basic.md
PASS mm --version
extra.md
PASS mm --version
pass 2
fail 0
```

## `mm tests/fixtures/passing --tags missing` (exit 1)

```expected
Error: No markdown test specs matched in $REPO_ROOT/tests/fixtures/passing
```
