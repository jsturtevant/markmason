# Use paths without hardcoding them

Path variables work in commands and expected output.
The runner creates a separate `E2E_HOME` and configuration directory for each
Markdown document.

## Locate isolated home and configuration directories

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
