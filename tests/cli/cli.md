# Get started with Markmason

Find available options and check the installed version.

These examples run as tests. `expected` checks the full output;
`expected-contains` checks selected lines.

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

[Run tests](running.md) | [Filter tests](filtering.md) | [Error cases](errors.md)
