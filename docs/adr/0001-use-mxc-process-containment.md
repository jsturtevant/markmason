---
status: accepted
---

# Use MXC Process containment for the first sandbox backend

Markmason will use MXC's OS-native Process containment as its first opt-in
sandbox backend because it can preserve arbitrary host CLI, PTY, and persistent
Bash semantics across Linux, Windows, and macOS. MXC Hyperlight and HLUK execute
inside guest runtimes or Linux root filesystems, so they remain separate future
experiments rather than pretending to be transparent command backends. Denial
diagnostics are best-effort: Markmason prints exact remediation when MXC exposes
the denied resource and otherwise reports the effective policy without guessing.
