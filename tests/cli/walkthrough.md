---
tags: [tutorial, cli]
---

# Walk through a Markdown tutorial

Run the same commands as a normal test suite or inspect each result at your own
pace. From this checkout, build `mm` and the [test CLI](../support/README.md) with
`just test-markdown`, then put `target/debug` and `target/debug/examples` on `PATH`.

```sh
mm --step --home-env TEST_HOME
# Or select this file directly, without the picker:
mm --step --file-pattern '^walkthrough\.md$' --home-env TEST_HOME
```

In the picker, type `walkthrough` or use the arrow keys to select this file,
then press Enter. Esc cancels without running any commands.

Press Enter after each result to continue, or type q then Enter to stop.
The section headings below become the titles in the walkthrough.

## Start with a greeting

A short command gives you one expected result to compare with the real output.

```markmason
mm-test-cli echo "Hello, Markmason!"
```

```expected
Hello, Markmason!
```

## Build output one command at a time

There is a pause after each line, even inside one block. After the first command,
the walkthrough shows `first` as actual output so far, without checking the
whole-block assertion yet. After the second command, both lines are checked.

```markmason
mm-test-cli echo first
mm-test-cli echo second
```

```expected
first
second
```

## Keep application input separate from tutorial controls

The `stdin` block goes to the application. Your Enter and quit controls go to
Markmason instead, so answering the walkthrough cannot change the test's input.

```markmason
mm-test-cli cat
```

```stdin
input for the application
```

```expected
input for the application
```
