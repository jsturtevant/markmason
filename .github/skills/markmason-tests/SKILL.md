---
name: markmason-tests
description: >-
  Write Markdown CLI tests with Markmason (mm) that also read as documentation.
  Use when adding Markmason to another repository, extending tests/cli,
  documenting CLI behavior with executable examples, separating normal usage
  from error cases, or writing bounded self-tests for mm itself.
---

# Markmason tests that read like docs

## Start with the project

- Inspect the CLI, its build commands, existing tests, and README.
- Run the real commands before writing their expected output. Do not invent output.
- Use the installed `mm` binary and put the tested CLI on `PATH`.
  Do not add a Node adapter or require Cargo just to run a released binary.
- Keep the project's existing test directory and scripts where practical.
  Use `tests/cli` as a default, not a requirement.
- Match `--home-env` to the application's state-directory variable. Set
  `--env-prefix` when the project uses its own filter environment variables.
- Read [format and examples](references/format-and-examples.md) for syntax and pitfalls.

## Write for someone learning the CLI

- Give each file a plain title and a one- or two-sentence introduction saying
  what it covers. Explain the feature, not the mechanics of the test harness.
- Use one topic per file: getting started, running tests, filtering, or another
  feature relevant to the project.
- Put one short, useful explanation beside each command. Say what it does or
  why its output matters.
- Treat walkthroughs as lessons for humans, not scripts hidden in Markdown.
  Prefer familiar CLI commands and short, named steps over dense query languages,
  nested pipelines, or general-purpose selection algorithms.
- Put reusable prerequisite checks in a clearly named `setup.md`: inspect the
  connected system and verify named resources exist, even when inputs came from
  the environment. Keep setup readable as its own guide; do not duplicate shared
  checks in each scenario just to make them visible in step mode.
- Keep the prose brief and natural. Avoid filler, marketing language, and
  repeated "this test verifies" introductions.
- Use descriptive headings and put commands in `markmason` fenced blocks.
  Group related commands in one block, one per line. The block is one test:
  output is combined and the first nonzero exit stops it.
  Put assertions and input blocks after the command, before the next heading.
- Keep every Markdown file in the scanned spec directory executable.
  Put indexes, references, and templates elsewhere; a case-free README there fails.

## Separate normal usage from errors

- Keep the main guides focused on successful commands.
- Move bad arguments, invalid specs, failed setup, output mismatches, no-match
  errors, and failure-recovery checks into `errors.md` or a separate error topic.
- State at the top of error files that the failures are deliberate.
  Put `(exit N)` on the opening `markmason` fence and explain why an expected error can pass.
- Keep passing sample suites genuinely successful. Do not make normal docs
  print confusing `PASS` lines for deliberately failing commands.
- Preserve negative coverage when moving examples. Update affected totals,
  fixture tables, links, and file descriptions.

## Assert meaningful results

- Prefer exact `expected` output for short, stable results.
- Use `expected-contains` for selected whole lines, in order. It is not substring
  or regex matching, and unmatched trailing lines still fail.
- Use `expected-substring` for a stable literal phrase inside dynamic output.
  Prefer it over piping through `grep` or rewriting output just for an assertion.
- Use `expected-regex` only when the match needs a pattern, such as a generated
  ID or duration. Write Rust regex escapes directly in the fence.
- Do not replace readable expected output with a dense one-line regex. Prefer
  exact, whole-line, or literal-substring assertions that still teach the human
  output format. If regex is necessary, keep it focused on the dynamic fragment.
- Treat each substring or regex block as one assertion over the full output,
  not a list of independent lines. Preserve whitespace and path separators;
  only CRLF line endings normalize. Extra output before and after is allowed.
- Confirm the installed `mm` supports these fences before using them; older
  versions may ignore unrecognized assertion blocks and check only the exit code.
- Assert actual version output, not just a successful exit or help text saying
  "Print version". Hardcode the current version and document updating the
  assertion alongside the package version.
- Match platform-independent help text when usage banners differ, such as
  `mm` versus `mm.exe`. Do not weaken unrelated assertions.
- Use supported path variables instead of hardcoded temporary directories.
- Check error messages as well as exit codes. A missing executable must not
  accidentally satisfy a test meant to check an application error.
- Keep setup per file as a full command, such as `setup: python3 setup.py`.
  Do not introduce a suite-wide interpreter setting.
- Keep fixtures small and deterministic. Reuse existing helpers for stdin,
  interactive terminals, timeouts, and stateful behavior rather than adding a
  runtime dependency merely to generate test output.
- Use `markmason always` for cleanup that must run after ordinary blocks,
  including failure or walkthrough cancellation. Keep it safe for partial work.
  Put an expected exit after the modifier: `markmason always (exit N)`.
  Do not rely on cleanup when inputs, fixture creation, or setup failed.

## Explain filters accurately

- Describe `--file-pattern` as matching relative file paths only.
  A matching file runs all its cases.
- Describe `--test-name-pattern` as matching file paths or command text, not headings.
  A file match runs the whole file; a command match selects its whole block.
- Show whole-file and anchored command matches when documenting the difference.
- Explain that file filters narrow the selection first, and that selecting a
  command does not run earlier prerequisite cases.
- State that tag inclusion requires all listed tags; exclusion rejects any
  matching tag. Keep fixture tags and case counts visible near the examples.

## Keep self-tests finite

When testing Markmason itself:

```text
tests/cli -> mm tests/fixtures/<suite> -> leaf commands
```

- Keep fixture specs outside the directory containing the outer self-tests.
- Use leaf commands that exit, such as `mm --help`, `mm --version`, or an invalid
  CLI option. Never use bare `mm` or point a nested run back at the outer suite.
- Put broken fixtures in their own directories, outside the passing sample suite.
- Keep real process tests for PTYs, cancellation, and cleanup. Do not claim the
  Markdown suite covers behavior exercised only by the Rust integration tests.

## Verify and make the docs discoverable

- Build the tested CLI if needed and use absolute `PATH` entries when fixtures
  change directories.
- Run `mm PATH/TO/SPECS --list`, then the affected examples through the project's
  existing test command. For prose-only edits, preserve commands and assertions.
- Check rendered Markdown, fixture links, expected totals, and the reading order.
- Exercise new CLI behavior on the supported platforms. Account for path and
  executable-name differences without dropping assertions.
- Add the suite to the existing CI test entry point if it is not already there.
- Keep the README short. Link directly to the rendered guide files as real
  examples, and show the command that runs those same files from a checkout.
- Use a feature branch and PR for review. Do not push directly to the default
  branch unless explicitly instructed.
