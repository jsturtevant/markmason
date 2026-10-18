# Declare tutorial inputs

Frontmatter `inputs` maps environment variable names to plain-text questions.
Markmason reuses nonempty environment values. `--step` prompts for missing
values before any fixture or setup runs, including declarations in Markdown
prerequisites. Values remain scoped to that file's fixture and are inherited by
its commands. This is not automatic shell-variable inference.

```yaml
inputs:
  KUBECONFIG: Path to the kubeconfig for your disposable cluster
  STORAGE_CONFIG: Name of the existing storage configuration
```

Inputs are plain text, not secret/password prompts. Do not use them for secrets.
Runner-managed variables such as `REPO_ROOT`, `XDG_CONFIG_HOME`, and the chosen
`--home-env` cannot be declared as inputs.

## Prompt before shared setup

This real-terminal test clears both caller values and answers the root input
and its Markdown prerequisite's input.

```markmason
mm-test-cli with-env MARKMASON_TUTORIAL_NAME "" MARKMASON_TUTORIAL_REGION "" -- mm tests/fixtures/inputs/lesson.md --step --home-env TEST_HOME
```

```pty
match: Input MARKMASON_TUTORIAL_NAME
send: tutorial user{enter}
match: Input MARKMASON_TUTORIAL_REGION
send: lab{enter}
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
Input MARKMASON_TUTORIAL_NAME (required; q + Enter cancels):
Input MARKMASON_TUTORIAL_REGION (required; q + Enter cancels):
ACTUAL OUTPUT
| tutorial user
| lab
Walkthrough finished
Passed blocks: 1
Failed blocks: 0
```

## Reuse supplied values without prompting

```markmason
mm-test-cli with-env MARKMASON_TUTORIAL_NAME "tutorial user" MARKMASON_TUTORIAL_REGION lab -- mm tests/fixtures/inputs/lesson.md --step --home-env TEST_HOME
```

```pty
match: Enter: Finish walkthrough
send: {enter}
```

```expected-contains
Walkthrough finished
Passed blocks: 1
Failed blocks: 0
```

## Unattended runs fail instead of asking

```markmason (exit 1)
mm-test-cli with-env MARKMASON_TUTORIAL_NAME "" MARKMASON_TUTORIAL_REGION "" -- mm tests/fixtures/inputs/lesson.md
```

```expected-contains
FAIL setup: Missing required inputs: MARKMASON_TUTORIAL_NAME, MARKMASON_TUTORIAL_REGION. Set these environment variables or use --step to be prompted.
```

## Quit while supplying inputs

`q` then Enter or EOF cancels before any setup or application command.

```markmason
mm-test-cli with-env MARKMASON_TUTORIAL_NAME "" MARKMASON_TUTORIAL_REGION "" -- mm tests/fixtures/inputs/lesson.md --step
```

```pty
match: Input MARKMASON_TUTORIAL_NAME
send: q{enter}
```

```expected-contains
Walkthrough stopped
Passed blocks: 0
Failed blocks: 0
```
