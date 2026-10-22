# Use paths without hardcoding them

Path variables work in commands and expected output.
Each Markdown document gets a fresh temporary state directory, exposed as
`E2E_HOME` by default and removed when the document finishes.

## Locate isolated state and configuration directories

````markdown markmason
```markmason
mm-test-cli env E2E_HOME XDG_CONFIG_HOME
```

The environment and expected-output variables point to the same isolated directories.

```expected
$E2E_HOME
$XDG_CONFIG_HOME
```
````

## Find the working directory

````markdown markmason
```markmason
mm-test-cli cwd
```

`$REPO_ROOT` is the working directory, whether or not it contains a Git repository.

```expected
$REPO_ROOT
```
````

## Find the project name

````markdown markmason
```markmason
mm-test-cli project
```

`$PROJECT_NAME` is the last component of that directory's path.

```expected
$PROJECT_NAME
```
````

## Quote paths containing spaces

````markdown markmason
```markmason
mm-test-cli echo "$REPO_ROOT" "$PROJECT_NAME"
```

Quote path variables when passing them as arguments.

```expected
$REPO_ROOT $PROJECT_NAME
```
````

## Use an application's state-directory variable

````markdown markmason
```markmason
mm tests/fixtures/custom-state-dir --state-dir-env MYCLI_HOME
```

`MYCLI_HOME` now points to the managed temporary state directory.
The flag changes the variable name, not the directory's lifecycle.

```expected-contains
RUN  state.md
  RUN  Block 1/1 — Report the custom state directory
  ✓ PASS Block 1/1 — Report the custom state directory
Blocks: 1 passed, 0 failed (includes deferred)
```
````
