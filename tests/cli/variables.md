# Use paths without hardcoding them

Path variables work in commands and expected output.
These examples use `--home-env TEST_HOME` for the [test CLI](../support/README.md);
the default home variable is `E2E_HOME`.

## Locate isolated home and configuration directories

````markdown markmason
```markmason
mm-test-cli env TEST_HOME XDG_CONFIG_HOME
```

The environment and expected-output variables point to the same isolated directories.

```expected
$TEST_HOME
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

## Choose an application-specific home variable

````markdown markmason
```markmason
mm tests/fixtures/custom-home --home-env MYCLI_HOME
```

Rename the home variable to match the application.
[This spec](../fixtures/custom-home/home.md) checks both its environment and substitution.

```expected-contains
RUN  home.md
  RUN  Block 1/1 — Report the custom home directory
  ✓ PASS Block 1/1 — Report the custom home directory
Blocks: 1 passed, 0 failed (includes deferred)
```
````
