# Run commands in an MXC sandbox

Use `--sandbox mxc` to execute setup, commands, terminal interactions, Bash, and
deferred cleanup through the host's native MXC process-containment backend.
Network access is denied unless the operator permits a destination.
The expected failures below demonstrate denied permissions. This guide is
separate from `tests/cli` because its nested `mm` commands require MXC.

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

First run without permission. The command fails because the working directory is
hidden and the fixture path is not mounted.

```markmason (exit 1)
mm tests/fixtures/sandbox-guide --cwd . --sandbox mxc --sandbox-workdir none
```

```expected-substring
Sandbox filesystem access may be denied; permit the path with --sandbox-read PATH or --sandbox-write PATH.
```

Now add the repository as an explicit read-only mount.

```markmason
mm tests/fixtures/sandbox-guide --cwd . --sandbox mxc --sandbox-workdir none --sandbox-read .
```

`--cwd .` selects this repository. `--sandbox-workdir none` hides it;
`--sandbox-read .` restores read-only access.
Additional `--sandbox-read` and `--sandbox-write` paths are operator-controlled.

```expected-contains
RUN  read.md
  ✓ PASS Block 1/1 — Read an approved file
Blocks: 1 passed, 0 failed (includes deferred)
```

## Deny a network connection

Without a network permission, this fixture's TCP connection fails inside MXC.
`203.0.113.1` is a documentation-only TEST-NET-3 address, not a real service.
The denial does not depend on a live server.

```markmason (exit 1)
mm tests/fixtures/sandbox-network --sandbox mxc
```

```expected-substring
Sandbox network access may be denied; permit the destination with --sandbox-net-allow HOST[:PORT].
```

## Allow one network destination

Permit outbound TCP access with a host or IP and optional port:

```markmason
mm tests/fixtures/passing --sandbox mxc --sandbox-net-allow 203.0.113.1:443
```

This syntax example uses the same documentation address. The passing suite makes
no connection; allowing an address does not create a service there.

```expected-contains
RUN  basic.md
  ✓ PASS Block 1/2 — Show help
RUN  extra.md
  ✓ PASS Block 1/1 — Show the installed version
Blocks: 3 passed, 0 failed (includes deferred)
```

Malformed permissions fail before the command runs:

```markmason (exit 1)
mm tests/fixtures/passing --sandbox mxc --sandbox-net-allow '[::1]443'
```

```expected-substring
Invalid sandbox network destination [::1]443: expected ':' after ']'
```

Denied connections retain the application's error and include
`--sandbox-net-allow HOST[:PORT]` guidance when the backend cannot report a more
specific destination. The ignored
`mxc_sandbox_denies_and_allows_network_access` Rust integration test exercises
both denied and allowed live connections without making the default test suite
depend on public network access.
