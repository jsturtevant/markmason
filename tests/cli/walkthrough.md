---
tags: [tutorial, cli]
---

# Walk through a Markdown tutorial

Run the same commands as a normal test suite or inspect each result at your own
pace. From this checkout, build `mm` and the [test CLI](../support/README.md) with
`just test-markdown`, then put `target/debug` and `target/debug/examples` on `PATH`.

```sh
mm --step
# Or select this file directly, without the picker:
mm --step --file-pattern '^walkthrough\.md$'
```

In the picker, type `walkthrough` or use the arrow keys to select this file,
then press Enter. Esc cancels without running any commands.

The current Markdown section stays in view: heading, description, full code
block, expected output, and collecting actual output. Completed commands keep
green `[done]` prefixes; the arrow points to the next command.
Nothing is collapsed or truncated. If a section does not fit, scroll with
Up/Down, PgUp/PgDn, Home/End, or the mouse wheel; scrolling does not run commands.
Keep sections short so they normally fit. The output area resets for each block.

Press Enter after each result to continue, b then Enter to run the rest of the
block and stop before the next block, f then Enter to finish the file, or q then
Enter to stop. Running ahead still checks every assertion and pauses on failure.
The section headings below become the titles in the walkthrough.
The [walkthrough tests](stepping.md) exercise `mm --step` itself using a real
terminal and scripted keypresses.

## Start with a greeting

A short command gives you one expected result to compare with the real output.

````markdown markmason
```markmason
mm-test-cli echo "Hello, Markmason!"
```

```expected
Hello, Markmason!
```
````

## Build output one command at a time

There is a pause after each line, even inside one block. After the first command,
the walkthrough shows `first` as actual output so far, without checking the
whole-block assertion yet. After the second command, both lines are checked.

````markdown markmason
```markmason
mm-test-cli echo first
mm-test-cli echo second
```

```expected
first
second
```
````

## Keep application input separate from tutorial controls

The `stdin` block goes to the application. Your Enter and quit controls go to
Markmason instead, so answering the walkthrough cannot change the test's input.

````markdown markmason
```markmason
mm-test-cli cat
```

```stdin
input for the application
```

```expected
input for the application
```
````
