# Clone a repository fixture

Use `clone: owner/repo` in frontmatter when a guide needs files from a GitHub
repository. An ordinary run checks `gh` authentication and clones the repository
before setup and commands begin.

## Preview the cloned fixture

```markmason
mm tests/fixtures/clone-list --list
```

`--list` shows the planned command without authenticating or cloning, so this
example remains safe to run offline.

```expected
clone.md
  Block 1/1 — Use a cloned repository
    mm-test-cli cloned
```
