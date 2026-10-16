# Run Markdown tests

Run a directory of specs or preview its commands without executing them.

Run these examples from the repository root with `mm` on `PATH`.
They use [sample specs](../fixtures/passing); bare `mm` defaults to `tests/cli`.

## `mm tests/fixtures/passing`

Run every `.md` file in the directory. `basic.md` has two cases;
`extra.md` has one.

```expected
basic.md
PASS mm --help
PASS mm --version
extra.md
PASS mm --version
pass 3
fail 0
```

With no failures, `mm` exits with code 0.

## `mm tests/fixtures/passing --list`

Preview commands without running them.

```expected
basic.md
mm --help
mm --version
extra.md
mm --version
```

No recursion: these fixtures only call help or version, never this guide's directory.

[Filter tests](filtering.md) | [Error cases](errors.md)
