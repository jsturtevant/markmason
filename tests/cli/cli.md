# Get started with Markmason

Find available options and check the installed version.

Each `markmason` block runs its command; the headings describe what it does.
`expected` checks the full output; `expected-contains` checks selected lines.

## Find available options

````markdown markmason
```markmason
mm --help
```

List the available options. Only the help and version entries are shown here.

```expected-contains
Standalone Markdown-driven CLI end-to-end test runner
-h, --help
Print help
-V, --version
Print version
```
````

## Check the installed version

````markdown markmason
```markmason
mm --version
```

Print the installed version, not the help text describing the flag.

```expected
mm 0.6.0
```
````

[Run tests](running.md) | [Filter tests](filtering.md) | [Error cases](errors.md)
