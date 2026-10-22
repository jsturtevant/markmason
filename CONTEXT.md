# Markmason

Markmason executes Markdown-described CLI scenarios while keeping each document's
state and lifecycle isolated from other documents.

## Language

**Execution backend**:
The mechanism that starts and controls a document's commands. The native backend
uses host processes; the MXC backend uses OS-native process containment.
_Avoid_: Runner, engine

**Sandbox policy**:
The operator-controlled ceiling on a sandboxed document's filesystem and network
access. Markdown may preserve or narrow this policy, but cannot broaden it.
_Avoid_: Permissions, sandbox config

**Effective working directory**:
The directory in which a document's commands run: the invocation directory by
default, an explicit `--cwd`, or a temporary cloned checkout.
_Avoid_: Spec directory, repository root

**Fixture**:
The per-document execution state: effective working directory, isolated home and
configuration, environment, processes, shell session, and optional clone.
_Avoid_: Test environment, sandbox
