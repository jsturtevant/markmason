# Run cleanup at the end

Use `markmason always` for work that must happen after a file finishes, even when
ordinary execution fails or a walkthrough stops. Cleanup runs automatically.

## See which blocks are deferred

The [example file](../fixtures/always/lesson.md) puts cleanup first in the document.
Listing shows ordinary commands first and labels cleanup as `Always`.

```markmason
mm tests/fixtures/always/lesson.md --list
```

```expected-contains
Always 3/3 — Remove the state at the end
cleanup_example
```

## Use the same Bash state

The ordinary blocks define a cleanup function, create state, and read it.
The deferred block calls that function in the same Bash session at the end.

```markmason
mm tests/fixtures/always/lesson.md --home-env TEST_HOME
```

```expected-contains
RUN  Always 3/3 — Remove the state at the end
✓ PASS Always 3/3 — Remove the state at the end
Documents: 1 passed, 0 failed
Blocks: 3 passed, 0 failed (includes deferred)
```

Use `bash markmason always` for Markdown language styling. Add `shell: bash`
in frontmatter when commands need Bash variables or functions. To declare an
expected exit, put it last: `markmason always (exit 2)`.

## Quit without skipping cleanup

Quit after defining the function, before creating the state. Cleanup is safe
even when there is no state to remove, and it asks for no further confirmation.

```markmason
mm tests/fixtures/always/lesson.md --step --home-env TEST_HOME
```

```stdin
q
```

```expected-contains
RUN  Always 3/3 — Remove the state at the end
✓ PASS Always 3/3 — Remove the state at the end
Walkthrough stopped
Documents: 0 passed, 0 failed, 1 cancelled
Blocks: 1 passed, 0 failed (includes deferred)
```

All cleanup blocks are attempted in document order, even if one fails.
They are skipped if fixture or setup creation failed. Filters must select an
ordinary block; matching only a cleanup command does not select the file.

The Rust process tests cover command failures, timeouts, Ctrl+C, EOF, and
terminal behavior that this short guide does not exercise.
