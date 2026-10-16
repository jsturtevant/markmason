# Command-line interface

Check shared help text, not the platform-specific `mm` / `mm.exe` usage line.

## `mm --help`

```expected-contains
Standalone Markdown-driven CLI end-to-end test runner
-h, --help
Print help
-V, --version
Print version
```

## `mm --version`

```expected
mm 0.1.0
```

## `mm --not-an-option` (exit 2)

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## `mm --timeout 0` (exit 2)

```expected-contains
error: invalid value '0' for '--timeout <TIMEOUT>': timeout must be positive, finite seconds
For more information, try '--help'.
```
