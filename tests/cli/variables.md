# Use paths without hardcoding them

Path variables work in command headings and expected output.
These examples use `--home-env TEST_HOME` for the [test CLI](../support/README.md);
the default home variable is `E2E_HOME`.

## `mm-test-cli env TEST_HOME XDG_CONFIG_HOME`

The environment and expected-output variables point to the same isolated directories.

```expected
$TEST_HOME
$XDG_CONFIG_HOME
```

## `mm-test-cli cwd`

`$REPO_ROOT` is the working directory, whether or not it contains a Git repository.

```expected
$REPO_ROOT
```

## `mm-test-cli project`

`$PROJECT_NAME` is the last component of that directory's path.

```expected
$PROJECT_NAME
```

## `mm-test-cli echo "$REPO_ROOT" "$PROJECT_NAME"`

Quote path variables when passing them as arguments.

```expected
$REPO_ROOT $PROJECT_NAME
```

## `mm tests/fixtures/custom-home --home-env MYCLI_HOME`

Rename the home variable to match the application.
[This spec](../fixtures/custom-home/home.md) checks both its environment and substitution.

```expected
home.md
PASS mm-test-cli env MYCLI_HOME
pass 1
fail 0
```
