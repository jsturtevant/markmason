# Use paths without hardcoding them

Path variables work in commands and expected output.
These examples use `--home-env TEST_HOME` for the [test CLI](../support/README.md);
the default home variable is `E2E_HOME`.

## Locate isolated home and configuration directories

```markmason
mm-test-cli env TEST_HOME XDG_CONFIG_HOME
```

The environment and expected-output variables point to the same isolated directories.

```expected
$TEST_HOME
$XDG_CONFIG_HOME
```

## Find the working directory

```markmason
mm-test-cli cwd
```

`$REPO_ROOT` is the working directory, whether or not it contains a Git repository.

```expected
$REPO_ROOT
```

## Find the project name

```markmason
mm-test-cli project
```

`$PROJECT_NAME` is the last component of that directory's path.

```expected
$PROJECT_NAME
```

## Quote paths containing spaces

```markmason
mm-test-cli echo "$REPO_ROOT" "$PROJECT_NAME"
```

Quote path variables when passing them as arguments.

```expected
$REPO_ROOT $PROJECT_NAME
```

## Choose an application-specific home variable

```markmason
mm tests/fixtures/custom-home --home-env MYCLI_HOME
```

Rename the home variable to match the application.
[This spec](../fixtures/custom-home/home.md) checks both its environment and substitution.

```expected-regex
(?s)RUN  home\.md.*✓ PASS  home\.md  1/1 blocks.*Documents: 1 passed, 0 failed.*Blocks: 1 passed, 0 failed
```
