---
shell: bash
setup: mm ../fixtures/shell/sharedset.md
---

# Keep a Bash session across commands

Add `shell: bash` to run commands in one persistent Bash session.
Variables, exports, functions, and `cd` changes remain available across blocks,
including Markdown setup. Without this field, commands still execute directly.

Use Bash on Linux or macOS, or Git Bash on native Windows. Bash 3.2, included
with macOS, is sufficient. Your login shell does not need to be Bash.

## Read values prepared by another Markdown file

The [shared setup](../fixtures/shell/sharedset.md) only sets variables and
defines a function. It does not create infrastructure or need an environment file.

````markdown markmason
```markmason
describe_session
printf '%s\n' "$LESSON"
```

```expected
shared setup is ready
persistent Bash
```
````

## Change variables and the working directory

Commands execute in the shell itself, not in a subshell for each step.

````markdown markmason
```markmason
MESSAGE=$(printf 'state survives')
mkdir -p "$E2E_HOME/working directory"
cd "$E2E_HOME/working directory"
printf '%s\n' "$MESSAGE"
```

```expected
state survives
```
````

## Use state from an earlier block

An exported variable is also visible to native child executables.

````markdown markmason
```markmason
test "$PWD" -ef "$E2E_HOME/working directory"
export MESSAGE
mm-test-cli env MESSAGE
```

```expected
state survives
```
````

## Step over a complete loop or pipeline

With `--step`, this block has two commands: the complete loop and the pipeline.
Indented lines and shell continuations keep their meaning.

````markdown markmason
```markmason
for word in one two; do
  printf '%s\n' "$word"
done
printf '%s\n' three \
  four | mm-test-cli cat
```

```expected
one
two
three
four
```
````

## Send input to the application, not the shell runner

`stdin` is still useful for application data. It is not needed to supply
shell scripts: write those directly in `markmason` blocks.

````markdown markmason
```markmason
read -r ANSWER
printf '%s\n' "$ANSWER"
```

```stdin
application input
```

```expected
application input
```
````

## Wait for background output

Wait for a background job before checking its output. Output printed while a
walkthrough is paused is retained for the next command, not discarded.

````markdown markmason
```markmason
(sleep 0.1; printf 'background ready\n') &
JOB=$!
wait "$JOB"
```

```expected
background ready
```
````

## Confirm that input state also survives

````markdown markmason
```markmason
printf '%s\n' "$ANSWER"
```

```expected
application input
```
````
