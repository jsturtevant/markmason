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

First run without a grant. The command fails because the working directory is
hidden and the fixture path is not mounted.

```markmason (exit 1)
mm tests/fixtures/sandbox-guide --sandbox mxc --sandbox-workdir none
```

```expected-substring
Sandbox filesystem access may be denied; grant the path with --sandbox-read PATH or --sandbox-write PATH.
```

Now add the repository as an explicit read-only mount.

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

## Allow one network destination

Network access is denied by default. Grant outbound TCP access with a host or IP
and optional port:

```sh
mm tests/network \
  --sandbox mxc \
  --sandbox-net-allow api.example.com:443
```

Malformed grants fail before the command runs:

```markmason (exit 1)
mm tests/fixtures/passing --sandbox mxc --sandbox-net-allow '[::1]443'
```

```expected-substring
Invalid sandbox network destination [::1]443: expected ':' after ']'
```

Denied connections retain the application's error and include
`--sandbox-net-allow HOST[:PORT]` guidance when the backend cannot report a more
specific destination. The opt-in Rust integration test exercises both denied and
allowed live connections without making the default test suite depend on public
network access:

```sh
cargo test --test runner mxc_sandbox_denies_and_allows_network_access -- --ignored
```
