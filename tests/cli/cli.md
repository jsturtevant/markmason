# Get started with Markmason

Find available options, check the installed version, and see how `mm` reports
unknown options and invalid timeouts.

These examples run as tests. `expected` checks the full output;
`expected-contains` checks selected lines. `(exit N)` sets the expected exit code.

## `mm --help`

List the available options. Only the help and version entries are shown here.

```expected-contains
Standalone Markdown-driven CLI end-to-end test runner
-h, --help
Print help
-V, --version
Print version
```

## `mm --version`

Print the installed version, not the help text describing the flag.

```expected
mm 0.1.0
```

## `mm --not-an-option` (exit 2)

Unknown options exit with code 2. No tests run.

```expected-contains
error: unexpected argument '--not-an-option' found
For more information, try '--help'.
```

## `mm --timeout 0` (exit 2)

Timeouts are in seconds and must be greater than zero.

```expected-contains
error: invalid value '0' for '--timeout <TIMEOUT>': timeout must be positive, finite seconds
For more information, try '--help'.
```

[Run tests](running.md) | [Filter tests](filtering.md)
