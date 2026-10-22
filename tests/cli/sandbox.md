# Run commands in an MXC sandbox

Use `--sandbox mxc` to execute setup, commands, terminal interactions, Bash, and
deferred cleanup through the host's native MXC process-containment backend.
Network access is denied unless the operator grants a destination.

## Run a normal suite in the sandbox

```markmason
mm tests/fixtures/passing --sandbox mxc
```

Existing Markdown commands and assertions keep their usual behavior.

```expected-contains
RUN  basic.md
  ✓ PASS Block 1/2 — Show help
RUN  extra.md
  ✓ PASS Block 1/1 — Show the installed version
Blocks: 3 passed, 0 failed (includes deferred)
```

## Expose only an approved read path

```markmason
mm tests/fixtures/sandbox-guide --sandbox mxc --sandbox-workdir none --sandbox-read .
```

`--cwd` selects where commands run. `--sandbox-workdir` separately controls
whether that directory is mounted read-write, read-only, or not at all.
Additional `--sandbox-read` and `--sandbox-write` paths are operator-controlled.

```expected-contains
RUN  read.md
  ✓ PASS Block 1/1 — Read an approved file
Blocks: 1 passed, 0 failed (includes deferred)
```
