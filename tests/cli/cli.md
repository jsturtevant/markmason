# Command-line interface

## `mm --help`

```expected-contains
Usage: mm [OPTIONS] [SPEC_DIR]
-h, --help
Print help
-V, --version
Print version
```

## `mm --version`

No version is hardcoded, so this remains valid after a release bump.

## `mm --not-an-option` (exit 2)

```expected-contains
error: unexpected argument '--not-an-option' found
Usage: mm [OPTIONS] [SPEC_DIR]
For more information, try '--help'.
```

## `mm --timeout 0` (exit 2)

```expected-contains
error: invalid value '0' for '--timeout <TIMEOUT>': timeout must be positive, finite seconds
For more information, try '--help'.
```
