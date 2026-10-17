use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Output, Stdio},
    sync::{OnceLock, mpsc},
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn helper() -> &'static Path {
    static HELPER: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    &HELPER
        .get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/test_cli.rs");
            let binary = dir
                .path()
                .join(format!("app{}", std::env::consts::EXE_SUFFIX));
            let output = Command::new("rustc")
                .arg("--edition=2024")
                .arg(&source)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            (dir, binary)
        })
        .1
}

fn command(specs: &Path) -> Command {
    command_with_home(specs, "TEST_HOME")
}

fn command_with_home(specs: &Path, home_env: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mm"));
    let path = std::env::join_paths(
        std::iter::once(helper().parent().unwrap().to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    command
        .arg(specs)
        .args(["--home-env", home_env])
        .env("PATH", path)
        .env("GH_CONFIG_DIR", specs.join("real-gh-config"))
        .env_remove("E2E_FILE_PATTERN")
        .env_remove("E2E_FILE_EXCLUDE")
        .env_remove("E2E_TAGS")
        .env_remove("E2E_TAGS_EXCLUDE")
        .env_remove("VERBOSE");
    command
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn step_process(
    specs: &Path,
    args: &[&str],
) -> (Child, ChildStdin, mpsc::Receiver<io::Result<String>>) {
    let mut command = command(specs);
    command.arg("--step");
    if !args
        .iter()
        .any(|arg| *arg == "--file-pattern" || arg.starts_with("--file-pattern="))
    {
        command.args(["--file-pattern", "."]);
    }
    let mut child = command
        .args(args)
        .env("NO_COLOR", "1")
        .env_remove("CLICOLOR_FORCE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::BufReader::new(stdout).lines() {
            if send.send(line.map(|line| line + "\n")).is_err() {
                break;
            }
        }
    });
    (child, stdin, receive)
}

fn step_output(specs: &Path, args: &[&str], input: &str) -> Output {
    let (child, mut stdin, receive) = step_process(specs, args);
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    let mut output = child.wait_with_output().unwrap();
    output.stdout = receive
        .into_iter()
        .map(Result::unwrap)
        .collect::<String>()
        .into_bytes();
    output
}

fn next_step(
    child: &mut Child,
    receive: &mpsc::Receiver<io::Result<String>>,
    transcript: &mut String,
) {
    loop {
        match receive.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(line)) => {
                let prompt = line.starts_with("Enter:");
                transcript.push_str(&line);
                if prompt {
                    return;
                }
            }
            error => {
                if child.try_wait().unwrap().is_none() {
                    child.kill().unwrap();
                }
                child.wait().unwrap();
                panic!("Walkthrough did not reach a pause: {error:?}\n{transcript}");
            }
        }
    }
}

#[test]
fn single_file_input_lists_without_setup_and_respects_filters() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lesson.md");
    fs::write(
        &file,
        "---\nsetup: missing-executable\n---\n```markmason\napp echo selected\n```\n",
    )
    .unwrap();
    fs::write(dir.path().join("other.md"), "```markmason\napp fail\n```\n").unwrap();
    let output = command(&file).arg("--list").output().unwrap();
    let result = text(&output);
    assert!(output.status.success(), "{result}");
    assert!(
        result.contains("lesson.md\n  app echo selected"),
        "{result}"
    );
    assert!(!result.contains("other.md"), "{result}");
    let output = command(&file)
        .args(["--list", "--file-pattern", "^other"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("No markdown test specs matched"));
}

#[test]
fn markdown_setup_shares_home_config_cwd_and_custom_variables_and_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("prerequisites")).unwrap();
    fs::write(
        dir.path().join("prerequisites/first steps.md"),
        "---\nsetup: app mark ./setup-ran\n---\n\
         ```markmason\napp state-exists CUSTOM_HOME\n```\n```expected\nfalse\n```\n\
         ```markmason\napp mark \"$CUSTOM_HOME/state\"\n\
         app write-state XDG_CONFIG_HOME configured\n\
         app mark \"$REPO_ROOT/prepared\"\n```\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("gettingstarted.md"),
        "---\nsetup: 'mm \"prerequisites/first steps.md\"'\n---\n\
         ```markmason\napp state-exists CUSTOM_HOME\n```\n```expected\ntrue\n```\n",
    )
    .unwrap();
    let lesson = dir.path().join("lesson.md");
    fs::write(
        &lesson,
        "---\nsetup: mm gettingstarted.md\n---\n\
         ```markmason\napp state-exists CUSTOM_HOME\n\
         app state-exists XDG_CONFIG_HOME\n```\n```expected\ntrue\ntrue\n```\n\
         ```markmason\napp env CUSTOM_HOME XDG_CONFIG_HOME\n```\n",
    )
    .unwrap();
    let output = command_with_home(&lesson, "CUSTOM_HOME")
        .arg("--cwd")
        .arg(dir.path())
        .args(["--verbose", "--test-name-pattern", "^lesson"])
        .output()
        .unwrap();
    let result = text(&output);
    assert!(output.status.success(), "{result}");
    assert!(result.contains("pass 2\nfail 0"), "{result}");
    assert!(dir.path().join("prepared").is_file(), "{result}");
    assert!(dir.path().join("prerequisites/setup-ran").is_file());
    let homes: Vec<_> = result
        .lines()
        .filter(|line| line.contains("markmason-home-") || line.contains("markmason-xdg-"))
        .filter(|line| Path::new(line).is_absolute())
        .collect();
    assert!(homes.len() >= 2, "{result}");
    for home in homes {
        assert!(!Path::new(home).exists(), "Fixture not cleaned up: {home}");
    }
}

#[test]
fn markdown_setup_failures_skip_later_setup_and_walkthrough_commands() {
    for (prerequisite, expected) in [
        ("```markmason\napp fail\n```\n", "expected exit 0"),
        (
            "```markmason\napp echo actual\n```\n```expected\ndifferent\n```\n",
            "Line 1 mismatch",
        ),
        (
            "```markmason\napp env TEST_HOME XDG_CONFIG_HOME\n```\n```expected\ndifferent\n```\n",
            "Line 1 mismatch",
        ),
        (
            "---\ntimeout: invalid\n---\n```markmason\napp echo no\n```\n",
            "Invalid Markdown setup",
        ),
        ("# No commands\n", "No test cases found in Markdown setup"),
        (
            "---\nclone: owner/repo\n---\n```markmason\napp echo no\n```\n",
            "cannot use clone",
        ),
        (
            "---\nsetup: mm lesson.md\n---\n```markmason\napp echo no\n```\n",
            "Circular Markdown setup",
        ),
        (
            "---\nsetup: mm missing.md\n---\n```markmason\napp echo no\n```\n",
            "Cannot find Markdown setup",
        ),
        (
            "---\ntimeout: 0.1\n---\n```markmason\napp sleep\n```\n",
            "timed out",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("must-not-run");
        let lesson = dir.path().join("lesson.md");
        fs::write(
            &lesson,
            format!(
                "---\nsetup: mm setup.md\n---\n```markmason\napp mark \"{}\"\n```\n",
                marker.display()
            ),
        )
        .unwrap();
        let setup = if prerequisite == "# No commands\n" {
            prerequisite.to_owned()
        } else {
            format!(
                "{prerequisite}\n```markmason\napp mark \"{}\"\n```\n",
                marker.display()
            )
        };
        fs::write(dir.path().join("setup.md"), setup).unwrap();
        let output = command(&lesson).output().unwrap();
        let result = text(&output);
        assert!(!output.status.success(), "{result}");
        assert!(result.contains(expected), "{expected}: {result}");
        assert!(!marker.exists(), "{result}");
        assert!(result.contains("pass 0\nfail 1"), "{result}");
        for home in result.lines().filter(|line| {
            Path::new(line).is_absolute()
                && (line.contains("markmason-home-") || line.contains("markmason-xdg-"))
        }) {
            assert!(!Path::new(home).exists(), "Fixture not cleaned up: {home}");
        }
    }
}

#[test]
fn markdown_setup_preserves_expected_nonzero_and_isolation_between_callers() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("setup.md"),
        "```markmason\napp state-exists\n```\n```expected\nfalse\n```\n\
         ```markmason (exit 2)\napp fail\n```\n```expected\nstdout\nstderr\n```\n\
         ```markmason\napp write-state TEST_HOME ready\n```\n",
    )
    .unwrap();
    for name in ["a.md", "b.md"] {
        fs::write(
            dir.path().join(name),
            "---\nsetup: mm setup.md\n---\n```markmason\napp state\n```\n\
             ```expected\nready\n```\n",
        )
        .unwrap();
    }
    let output = command(dir.path())
        .args(["--file-pattern", "^[ab]\\.md$"])
        .output()
        .unwrap();
    let result = text(&output);
    assert!(output.status.success(), "{result}");
    assert!(result.contains("pass 2\nfail 0"), "{result}");
}

#[test]
fn markdown_setup_shares_cloned_checkout_and_background_processes() {
    let dir = tempfile::tempdir().unwrap();
    fs::hard_link(
        helper(),
        dir.path()
            .join(format!("gh{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
    fs::write(
        dir.path().join("setup.md"),
        "---\nsetup: app mark prepared\n---\n\
         ```markmason\napp cloned\n```\n```expected\nyes\n```\n\
         ```markmason\napp background \"$REPO_ROOT/processes\"\n```\n",
    )
    .unwrap();
    let lesson = dir.path().join("lesson.md");
    fs::write(
        &lesson,
        "---\nclone: owner/repo\nsetup: mm setup.md\n---\n\
         ```markmason\napp await-agent\n```\n```expected\nready\n```\n\
         ```markmason\napp setup ./prepared\n```\n\
         ```markmason\napp cwd\n```\n",
    )
    .unwrap();
    let path = std::env::join_paths(
        [dir.path().to_owned(), helper().parent().unwrap().to_owned()]
            .into_iter()
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = command(&lesson)
        .env("PATH", path)
        .arg("--verbose")
        .output()
        .unwrap();
    let result = text(&output);
    assert!(output.status.success(), "{result}");
    assert!(result.contains("pass 3\nfail 0"), "{result}");
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("prepared")
            .exists()
    );
    let checkout = result
        .lines()
        .find(|line| Path::new(line).is_absolute() && line.contains("markmason-repo-"))
        .unwrap();
    assert!(
        !Path::new(checkout).exists(),
        "Cloned fixture not cleaned up: {checkout}"
    );
}

#[test]
fn step_mode_requires_a_terminal_or_an_explicit_file_filter() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("must-not-run");
    fs::write(
        dir.path().join("lesson.md"),
        format!(
            "---\nsetup: app mark \"{}\"\n---\n```markmason\napp fail\n```\n",
            marker.display()
        ),
    )
    .unwrap();
    let output = command(dir.path()).arg("--step").output().unwrap();
    let result = text(&output);
    assert!(!output.status.success(), "{result}");
    assert!(
        result.contains("picker needs an interactive terminal"),
        "{result}"
    );
    assert!(result.contains("--file-pattern REGEX"), "{result}");
    assert!(!marker.exists(), "{result}");
    assert!(!result.contains("MARKMASON WALKTHROUGH"), "{result}");
}

#[test]
fn step_mode_accepts_environment_file_filters_without_a_picker() {
    for prefix in ["E2E_", "TUTORIAL_"] {
        let dir = tempfile::tempdir().unwrap();
        let selected = dir.path().join("selected");
        let skipped = dir.path().join("skipped");
        for (name, marker) in [("chosen.md", &selected), ("other.md", &skipped)] {
            fs::write(
                dir.path().join(name),
                format!("```markmason\napp mark \"{}\"\n```\n", marker.display()),
            )
            .unwrap();
        }
        let output = command(dir.path())
            .args(["--step", "--env-prefix", prefix])
            .env(format!("{prefix}FILE_PATTERN"), "^chosen\\.md$")
            .output()
            .unwrap();
        let result = text(&output);
        assert!(output.status.success(), "{result}");
        assert!(selected.exists(), "{result}");
        assert!(!skipped.exists(), "{result}");
        assert!(result.contains("chosen.md  |  Block 1 of 1"), "{result}");
        assert!(
            !result.contains("Choose a Markdown walkthrough"),
            "{result}"
        );
    }
}

#[test]
fn step_mode_picker_searches_navigates_and_cancels_before_setup() {
    for (input, exit) in [
        ("selected{enter}", 0),
        ("{down}{enter}", 0),
        ("\x1b", 0),
        #[cfg(unix)]
        ("\x03", 130),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let lessons = dir.path().join("lessons");
        fs::create_dir_all(lessons.join("nested")).unwrap();
        for (name, tags) in [
            ("a-first.md", "tutorial"),
            ("nested/b selected.md", "tutorial"),
            ("other.md", "other"),
            ("ignored.md", "tutorial"),
        ] {
            let key = Path::new(name).file_stem().unwrap().to_str().unwrap();
            fs::write(
                lessons.join(name),
                format!(
                    "---\ntags: [{tags}]\nsetup: app mark \"{}\"\n---\n\
                     ```markmason\napp mark \"{}\"\n```\n",
                    dir.path().join(format!("{key}-setup")).display(),
                    dir.path().join(format!("{key}-command")).display(),
                ),
            )
            .unwrap();
        }
        let cancelled = matches!(input, "\x1b" | "\x03");
        let next = if cancelled {
            ""
        } else {
            "match: Enter: Finish walkthrough\nsend: {enter}\n"
        };
        fs::write(
            dir.path().join("driver.md"),
            format!(
                "```markmason (exit {exit})\n\"{}\" \"{}\" --step --home-env TEST_HOME \
                 --tags tutorial --file-exclude ignored --test-name-pattern \"^app mark\"\n```\n\
                 ```pty\nmatch: nested/b selected.md\nsend: {input}\n{next}```\n",
                env!("CARGO_BIN_EXE_mm"),
                lessons.display(),
            ),
        )
        .unwrap();
        let output = command(dir.path())
            .args(["--file-pattern", "^driver\\.md$", "--verbose"])
            .output()
            .unwrap();
        let result = text(&output);
        assert!(output.status.success(), "{result}");
        assert!(result.contains("Choose a Markdown walkthrough"), "{result}");
        assert!(!result.contains("other.md"), "{result}");
        assert!(!result.contains("ignored.md"), "{result}");
        for key in ["a-first", "other", "ignored"] {
            for phase in ["setup", "command"] {
                assert!(
                    !dir.path().join(format!("{key}-{phase}")).exists(),
                    "{result}"
                );
            }
        }
        for phase in ["setup", "command"] {
            assert_eq!(
                dir.path().join(format!("b selected-{phase}")).exists(),
                !cancelled,
                "{result}"
            );
        }
        if input == "\x1b" {
            assert!(
                result.contains("Walkthrough cancelled. No commands were run."),
                "{result}"
            );
            assert!(!result.contains("MARKMASON WALKTHROUGH"), "{result}");
        } else if cancelled {
            assert!(result.contains("Interrupted by signal 2"), "{result}");
            assert!(!result.contains("MARKMASON WALKTHROUGH"), "{result}");
        } else {
            assert!(
                result.contains("nested/b selected.md  |  Block 1 of 1"),
                "{result}"
            );
            assert!(result.contains("Walkthrough finished"), "{result}");
        }
    }
}

#[test]
fn step_mode_filters_files_and_checks_combined_output_after_the_block() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("lesson.md"),
        "---\ntags: [tutorial]\n---\n## Print **two** lines\n\n```markmason\napp echo first\napp echo second\n```\n```expected\nfirst\nsecond\n```\n",
    ).unwrap();
    fs::write(dir.path().join("other.md"), "```markmason\napp fail\n```\n").unwrap();
    let output = step_output(
        dir.path(),
        &[
            "--file-pattern",
            "^lesson\\.md$",
            "--tags",
            "tutorial",
            "--test-name-pattern",
            "^app echo second$",
            "--verbose",
        ],
        "\n\n",
    );
    let result = text(&output);
    assert!(output.status.success(), "{result}");
    for expected in [
        "MARKMASON WALKTHROUGH",
        "lesson.md  |  Block 1 of 1",
        "Print two lines",
        "COMMAND 1 OF 2",
        "$ app echo first",
        "EXPECTED OUTPUT (whole block; exact, exit 0)\n  | first\n  | second",
        "ACTUAL OUTPUT SO FAR\n  | first\n\n",
        "IN PROGRESS",
        "COMMAND 2 OF 2",
        "ACTUAL OUTPUT\n  | first\n  | second",
        "Finish walkthrough",
        "Walkthrough finished",
        "Passed blocks: 1\nFailed blocks: 0",
    ] {
        assert!(result.contains(expected), "missing {expected:?}\n{result}");
    }
    assert!(!result.contains("other.md"), "{result}");
    assert!(!result.contains("DIFF (expected vs actual)"), "{result}");
    assert!(!result.contains('\x1b'), "{result}");
}

#[test]
fn step_mode_waits_after_every_command_and_quits_mid_block() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    let third = dir.path().join("third");
    fs::write(
        dir.path().join("lesson.md"),
        format!(
            "```markmason\napp mark \"{}\"\napp mark \"{}\"\napp mark \"{}\"\n```\n",
            first.display(),
            second.display(),
            third.display()
        ),
    )
    .unwrap();
    let (mut child, mut stdin, receive) = step_process(dir.path(), &[]);
    let mut transcript = String::new();
    next_step(&mut child, &receive, &mut transcript);
    assert!(first.exists(), "{transcript}");
    assert!(!second.exists(), "{transcript}");
    stdin.write_all(b"not a key\n").unwrap();
    next_step(&mut child, &receive, &mut transcript);
    assert!(!second.exists(), "{transcript}");
    stdin.write_all(b"\n").unwrap();
    next_step(&mut child, &receive, &mut transcript);
    assert!(second.exists(), "{transcript}");
    assert!(!third.exists(), "{transcript}");
    stdin.write_all(b"q\n").unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    transcript.extend(receive.into_iter().map(Result::unwrap));
    assert!(output.status.success(), "{transcript}\n{}", text(&output));
    assert!(!third.exists(), "{transcript}");
    assert!(transcript.contains("Walkthrough stopped"), "{transcript}");
    assert!(
        transcript.contains("Passed blocks: 0\nFailed blocks: 0"),
        "{transcript}"
    );
    assert!(!transcript.contains("Input closed"), "{transcript}");
    assert!(!transcript.contains("COMMAND 3 OF 3"), "{transcript}");
}

#[test]
fn step_mode_quit_and_eof_skip_remaining_commands_and_clean_up_fixtures() {
    for input in ["q\n", ""] {
        let dir = tempfile::tempdir().unwrap();
        let later = dir.path().join("later");
        fs::write(
            dir.path().join("a.md"),
            format!(
                "```markmason\napp env TEST_HOME XDG_CONFIG_HOME\napp mark \"{}\"\n```\n",
                later.display()
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("b.md"),
            format!("```markmason\napp mark \"{}\"\n```\n", later.display()),
        )
        .unwrap();
        let output = step_output(dir.path(), &[], input);
        let result = text(&output);
        assert!(output.status.success(), "{result}");
        assert!(result.contains("Walkthrough stopped"), "{result}");
        assert!(!later.exists(), "{result}");
        assert!(!result.contains("b.md  |"), "{result}");
        let actual = result.split("ACTUAL OUTPUT SO FAR\n").nth(1).unwrap();
        let paths: Vec<_> = actual
            .lines()
            .filter_map(|line| line.strip_prefix("  | "))
            .take(2)
            .collect();
        assert_eq!(paths.len(), 2, "{result}");
        for path in paths {
            assert!(
                !Path::new(path).exists(),
                "Fixture survived quitting: {path}"
            );
        }
        assert_eq!(
            result.contains("Input closed"),
            input.is_empty(),
            "{result}"
        );
    }
}

#[test]
fn step_mode_shows_expected_errors_and_keeps_failure_status_after_continuing() {
    let dir = tempfile::tempdir().unwrap();
    let skipped = dir.path().join("skipped");
    fs::write(
        dir.path().join("lesson.md"),
        format!(
            "## An expected error\n```markmason (exit 2)\napp echo before\napp fail\napp mark \"{}\"\n```\n```expected\nbefore\nstdout\nstderr\n```\n\
             ## A mismatch\n```markmason\napp echo actual\n```\n```expected\ndifferent\n```\n\
             ## Continue after failure\n```markmason\napp echo after\n```\n```expected\nafter\n```\n",
            skipped.display()
        ),
    ).unwrap();
    let output = step_output(dir.path(), &[], "\n\n\n\n");
    let result = text(&output);
    assert!(!output.status.success(), "{result}");
    assert!(!skipped.exists(), "{result}");
    for expected in [
        "Command exited 2",
        "PASS  Block matched expectations.",
        "Line 1 mismatch",
        "Continue after failure",
        "Passed blocks: 2\nFailed blocks: 1",
    ] {
        assert!(result.contains(expected), "missing {expected:?}\n{result}");
    }
    assert!(!result.contains("COMMAND 3 OF 3"), "{result}");
}

#[test]
fn step_mode_reports_execution_errors_without_treating_them_as_expected_exits() {
    for (command, message) in [
        ("missing-executable-markmason", "Failed to start"),
        ("app sleep", "timed out"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("lesson.md"),
            format!("---\ntimeout: 0.15\n---\n```markmason (exit 1)\n{command}\n```\n"),
        )
        .unwrap();
        let output = step_output(dir.path(), &[], "q\n");
        let result = text(&output);
        assert!(!output.status.success(), "{result}");
        assert!(result.contains(message), "{result}");
        assert!(result.contains("OUTPUT CAPTURED BEFORE ERROR"), "{result}");
        assert!(
            result.contains("Passed blocks: 0\nFailed blocks: 1"),
            "{result}"
        );
    }
}

#[test]
fn step_mode_keeps_walkthrough_input_separate_from_stdin_and_terminal_scripts() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("lesson.md"),
        "```markmason\napp cat\napp echo end | app cat\n```\n```stdin\npayload\n```\n```expected\npayload\nend\n```\n\
         ```markmason\napp interactive\napp interactive\n```\n```pty\n\
         match: Ready?\nsend: yes{enter}\nmatch: Ready?\nsend: no{enter}\n\
         match: Ready?\nsend: first{enter}\nmatch: Ready?\nsend: last{enter}\n```\n\
         ```expected-contains\ndone no\ndone last\n```\n",
    ).unwrap();
    let output = step_output(dir.path(), &[], "\n\n\n\n");
    let result = text(&output);
    assert!(output.status.success(), "{result}");
    assert!(
        result.contains("Passed blocks: 2\nFailed blocks: 0"),
        "{result}"
    );
    assert!(
        result.contains("ACTUAL OUTPUT\n  | payload\n  | end"),
        "{result}"
    );
    assert!(result.contains("ordered lines"), "{result}");
    assert!(!result.contains('\x1b'), "{result}");
}

#[test]
fn step_mode_names_files_when_setup_fails_or_no_commands_exist() {
    for (markdown, expected) in [
        (
            "---\nsetup: app fail\n---\n```markmason\napp echo must-not-run\n```\n",
            "FAIL setup (lesson.md): Setup command failed",
        ),
        ("# No commands\n", "FAIL (lesson.md): No test cases found"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lesson.md"), markdown).unwrap();
        let output = step_output(dir.path(), &[], "");
        let result = text(&output);
        assert!(!output.status.success(), "{result}");
        assert!(result.contains(expected), "{result}");
        assert!(!result.contains("must-not-run"), "{result}");
        assert!(
            result.contains("Passed blocks: 0\nFailed blocks: 1"),
            "{result}"
        );
    }
}

#[test]
fn step_mode_cannot_be_combined_with_listing() {
    let dir = tempfile::tempdir().unwrap();
    let output = command(dir.path())
        .args(["--step", "--list"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert!(
        text(&output).contains("cannot be used with"),
        "{}",
        text(&output)
    );
}

#[cfg(unix)]
#[test]
fn step_mode_cancellation_while_paused_cleans_up_without_advancing() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let dir = tempfile::tempdir().unwrap();
        let later = dir.path().join("later");
        fs::write(
            dir.path().join("lesson.md"),
            format!(
                "```markmason\napp env TEST_HOME XDG_CONFIG_HOME\napp mark \"{}\"\n```\n",
                later.display()
            ),
        )
        .unwrap();
        let (mut child, stdin, receive) = step_process(dir.path(), &[]);
        let mut transcript = String::new();
        next_step(&mut child, &receive, &mut transcript);
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Walkthrough did not cancel while paused: {transcript}");
        }
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        transcript.extend(receive.into_iter().map(Result::unwrap));
        assert_eq!(
            output.status.code(),
            Some(128 + signal),
            "{transcript}\n{}",
            text(&output)
        );
        assert!(!later.exists(), "{transcript}");
        let actual = transcript.split("ACTUAL OUTPUT SO FAR\n").nth(1).unwrap();
        for path in actual
            .lines()
            .filter_map(|line| line.strip_prefix("  | "))
            .take(2)
        {
            assert!(
                !Path::new(path).exists(),
                "Fixture survived cancellation: {path}"
            );
        }
    }
}

#[test]
fn standalone_sequential_setup_variables_pipeline_and_nonzero_exit() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("setup.fixture"), "").unwrap();
    fs::write(
        dir.path().join("test.md"),
        r#"---
setup: app setup setup.fixture
---
```markmason
app state
```
```expected
ready
```
```markmason
app echo "hello | world" "" | app cat
```
```expected
hello | world
```
```markmason
app cat
```
```stdin
input
```
```expected
input
```
```markmason (exit 2)
app fail | app echo must-not-run
```
```expected
stdout
stderr
```
```markmason
app echo $TEST_HOME $REPO_ROOT $PROJECT_NAME $XDG_CONFIG_HOME
```
```expected
$TEST_HOME $REPO_ROOT $PROJECT_NAME $XDG_CONFIG_HOME
```
```markmason
app vars
```
```expected
$TEST_HOME
$XDG_CONFIG_HOME
"#
        .to_owned()
            + &dir.path().join("real-gh-config").display().to_string()
            + r#"
$REPO_ROOT
```
```markmason
app bulk
```
"#,
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("pass 7"), "{}", text(&output));
}

#[test]
fn command_blocks_combine_output_share_state_and_filter_as_one_test() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("commands.md"),
        r#"## Run commands in order

```markmason
app write-state TEST_HOME ready

app state
app echo "hello | world" | app cat
```
```expected
ready
hello | world
```

## Send stdin only to the first command

```markmason
app cat
app cat
```
```stdin
input once
```
```expected
input once
```

## An independent case

```markmason
app echo unselected
```
"#,
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("pass 3\nfail 0"),
        "{}",
        text(&output)
    );
    let output = command(dir.path())
        .args(["--test-name-pattern", "^app state$"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("pass 1\nfail 0"),
        "{}",
        text(&output)
    );
    assert!(!text(&output).contains("unselected"), "{}", text(&output));
    let output = command(dir.path()).arg("--list").output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains(
            "  app write-state TEST_HOME ready\n  app state\n  app echo \"hello | world\" | app cat"
        ),
        "{}",
        text(&output)
    );
}

#[test]
fn command_blocks_stop_on_failure_and_check_the_combined_output_and_exit() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("must not run");
    for failing in ["app fail", "app fail | app echo must-not-run"] {
        for (exit, expected, success) in [
            (2, "before\nstdout\nstderr", true),
            (0, "before\nstdout\nstderr", false),
            (2, "wrong output", false),
        ] {
            fs::write(
                dir.path().join("commands.md"),
                format!(
                    "```markmason (exit {exit})\napp echo before\n{failing}\napp mark \"{}\"\n```\n```expected\n{expected}\n```\n",
                    marker.display()
                ),
            )
            .unwrap();
            let output = command(dir.path()).output().unwrap();
            assert_eq!(output.status.success(), success, "{}", text(&output));
            assert!(!marker.exists(), "Commands continued after failure");
        }
    }
    fs::write(
        dir.path().join("commands.md"),
        format!(
            "```markmason (exit 1)\napp echo before\nmissing-executable-markmason\napp mark \"{}\"\n```\n",
            marker.display()
        ),
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(!output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("Failed to start"),
        "{}",
        text(&output)
    );
    assert!(!marker.exists(), "Commands continued after a startup error");
}

#[test]
fn command_blocks_share_terminal_steps_across_processes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("commands.md"),
        r#"```markmason
app echo before
app interactive
app interactive
app echo after
```
```pty
match: Ready?
send: yes{enter}
match: Ready?
send: no{enter}
match: Ready?
send: first{enter}
match: Ready?
send: last{enter}
```
```expected-contains
before
done no
done last
after
```
"#,
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("pass 1\nfail 0"),
        "{}",
        text(&output)
    );
    fs::write(
        dir.path().join("commands.md"),
        "```markmason\napp interactive\napp premature\n```\n```pty\nmatch: Ready?\nsend: yes{enter}\nmatch: Ready?\nsend: no{enter}\nmatch: missing\nsend: unused\n```\n",
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(!output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("before completing step"),
        "{}",
        text(&output)
    );
}

#[test]
fn literal_examples_do_not_execute_and_prose_does_not_drop_assertions() {
    let dir = tempfile::tempdir().unwrap();
    for prose in ["***\n---", "- Explanation\n---"] {
        for (expected, success) in [("wrong output", false), ("actual", true)] {
            fs::write(
                dir.path().join("markdown.md"),
                format!(
                    "- ~~~~markdown\n  ~~~markmason\n  missing-executable-markmason\n  ~~~\n  ~~~~\n\n\
                     ~~~markmason\napp echo actual\n~~~\n\n{prose}\n\n\
                     ~~~expected\n{expected}\n~~~\n"
                ),
            )
            .unwrap();
            let output = command(dir.path()).output().unwrap();
            let result = text(&output);
            assert_eq!(output.status.success(), success, "{result}");
            if success {
                assert!(result.contains("pass 1\nfail 0"), "{result}");
            } else {
                assert!(result.contains("Line 1 mismatch"), "{result}");
                assert!(result.contains("pass 0\nfail 1"), "{result}");
            }
            assert!(!result.contains("missing-executable-markmason"), "{result}");
        }
    }
}

#[test]
fn each_file_chooses_its_own_setup_command() {
    let dir = tempfile::Builder::new()
        .prefix("markdown e2e ")
        .tempdir()
        .unwrap();
    let executable = format!("setup tool{}", std::env::consts::EXE_SUFFIX);
    fs::hard_link(helper(), dir.path().join(&executable)).unwrap();
    fs::write(dir.path().join("setup.fixture"), "").unwrap();
    fs::write(
        dir.path().join("a.md"),
        format!("---\nsetup: '\"{executable}\" setup setup.fixture'\n---\n```markmason\napp state\n```\n```expected\nready\n```\n"),
    ).unwrap();
    fs::write(
        dir.path().join("b.md"),
        "---\nsetup: app setup-other setup.fixture\n---\n```markmason\napp state\n```\n```expected\nother\n```\n",
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("pass 2"), "{}", text(&output));
}

#[test]
fn setup_runner_flag_is_no_longer_supported() {
    let dir = tempfile::tempdir().unwrap();
    let output = command(dir.path())
        .args(["--setup-runner", "app"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        text(&output).contains("unexpected argument '--setup-runner'"),
        "{}",
        text(&output)
    );
}

#[test]
fn setup_executes_directly_without_a_node_interpreter() {
    let dir = tempfile::Builder::new()
        .prefix("markdown e2e ")
        .tempdir()
        .unwrap();
    let name = format!("setup{}", std::env::consts::EXE_SUFFIX);
    fs::hard_link(helper(), dir.path().join(&name)).unwrap();
    fs::write(
        dir.path().join("direct.md"),
        format!("---\nsetup: {name}\n---\n```markmason\napp state\n```\n```expected\nready\n```\n"),
    )
    .unwrap();
    let output = command(dir.path())
        .env("PATH", helper().parent().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
}

#[cfg(windows)]
#[test]
fn windows_batch_shims_work_in_pipes_and_ptys() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("shim.cmd"),
        format!("@echo off\r\n\"{}\" %*\r\n", helper().display()),
    )
    .unwrap();
    fs::write(
        dir.path().join("shim.md"),
        r#"```markmason
shim echo "hello & world"
```
```expected
hello & world
```
```markmason
shim interactive
```
```pty
match: Ready?
send: yes{enter}
match: Ready?
send: no{enter}
```
```expected
done no
```
"#,
    )
    .unwrap();
    let path = std::env::join_paths(
        [dir.path().to_owned(), helper().parent().unwrap().to_owned()]
            .into_iter()
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = command(dir.path()).env("PATH", path).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
}

#[test]
fn recursive_discovery_tags_files_and_name_filter_failures() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::write(
        dir.path().join("nested/slow.md"),
        "---\ntags: [Dispatch, SLOW]\n---\n```markmason\napp fail\n```\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("fast.md"),
        "```markmason\napp echo fast\n```\n",
    )
    .unwrap();
    let output = command(dir.path())
        .env("E2E_TAGS_EXCLUDE", "SLOW")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(!text(&output).contains("nested/slow.md"));
    let output = command(dir.path())
        .args(["--tags", "dispatch,slow", "--list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(text(&output).contains("nested/slow.md"));
    assert!(!text(&output).contains("fast.md"));
    let output = command(dir.path())
        .args([
            "--file-pattern",
            "^nested/",
            "--test-name-pattern",
            "slow.md",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(text(&output).contains("fail 1"));
    let output = command(dir.path())
        .args(["--test-name-pattern", "app fail"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let output = command(dir.path())
        .args(["--file-exclude", "nested", "--test-name-pattern", "fast"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = command(dir.path())
        .args(["--file-pattern", "missing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(text(&output).contains("No markdown test specs matched"));
}

#[test]
fn timeouts_are_not_expected_exit_failures() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("timeout.md"),
        "---\ntimeout: 0.15\n---\n```markmason (exit 1)\napp sleep\n```\n",
    )
    .unwrap();
    let start = Instant::now();
    let output = command(dir.path()).output().unwrap();
    assert!(!output.status.success());
    assert!(text(&output).contains("timed out"), "{}", text(&output));
    assert!(text(&output).contains("before timeout"));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn pty_raw_clean_repeated_prompts_and_missing_steps() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("interactive.md"),
        r#"```markmason
app interactive
```
```pty
match-raw: {hide-cursor}
send:
match: Ready?
send: yes{enter}
match: Ready?
send: no{enter}
```
```expected
done no
```
"#,
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    fs::write(
        dir.path().join("interactive.md"),
        "```markmason\napp premature\n```\n```pty\nmatch: missing\nsend: y\n```\n",
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(!output.status.success());
    assert!(
        text(&output).contains("before completing step"),
        "{}",
        text(&output)
    );
    fs::write(
        dir.path().join("interactive.md"),
        "---\ntimeout: 0.1\n---\n```markmason\napp sleep\n```\n```pty\nmatch: missing\nsend: y\n```\n",
    )
    .unwrap();
    let output = command(dir.path()).output().unwrap();
    assert!(!output.status.success());
    assert!(
        text(&output).contains("PTY command timed out"),
        "{}",
        text(&output)
    );
}

#[test]
fn pty_failure_diagnostics_strip_terminal_control_sequences() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("errors.md"),
        r#"```markmason
app interactive
```
```pty
match: missing
send: unused{enter}
```

```markmason
app interactive
```
```pty
match: Ready?
send: yes{enter}
match: Ready?
send: no{enter}
match: missing
send: unused{enter}
```
"#,
    )
    .unwrap();
    let output = command(dir.path())
        .args(["--timeout", "2"])
        .output()
        .unwrap();
    let result = text(&output);
    assert_eq!(output.status.code(), Some(1), "{result}");
    for expected in [
        "pass 0\nfail 2",
        "PTY command timed out after 2000ms.",
        "Waiting for step 0: match \"missing\"",
        "Output so far:\nReady?",
        "PTY exited before completing step 2: match \"missing\"",
        "accepted yes",
        "done no",
    ] {
        assert!(
            result.contains(expected),
            "missing {expected:?}: {result:?}"
        );
    }
    assert!(
        !result.contains('\x1b'),
        "terminal codes leaked: {result:?}"
    );
}

#[test]
fn invalid_setup_spawn_empty_specs_and_mismatches_fail() {
    let dir = tempfile::tempdir().unwrap();
    for (markdown, expected) in [
        ("# no commands", "No test cases found"),
        (
            "## `app echo legacy`\n",
            "Expected a fenced markmason command block",
        ),
        (
            "```markmason\napp echo unterminated",
            "Unterminated markmason block",
        ),
        ("---\ntags: nope\n---\n", "tags must be an array"),
        (
            "```markmason (exit 1)\nmissing-executable-markmason\n```",
            "Failed to start",
        ),
        (
            "```markmason\napp echo actual\n```\n```expected\nexpected\n```\n",
            "Line 1 mismatch",
        ),
        (
            "---\nsetup: missing\n---\n```markmason\napp echo ignored\n```",
            "Setup command failed",
        ),
        (
            "---\nsetup: missing-executable-markmason setup.fixture\n---\n```markmason\napp echo ignored\n```",
            "Setup command failed",
        ),
        (
            "---\nsetup: app fail\n---\n```markmason\napp echo ignored\n```",
            "Setup command failed: app fail",
        ),
    ] {
        fs::write(dir.path().join("test.md"), markdown).unwrap();
        let output = command(dir.path()).output().unwrap();
        assert!(!output.status.success(), "{markdown}");
        assert!(text(&output).contains(expected), "{}", text(&output));
    }
}

#[test]
fn clone_fixture_is_isolated_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let gh = dir
        .path()
        .join(format!("gh{}", std::env::consts::EXE_SUFFIX));
    fs::hard_link(helper(), &gh).unwrap();
    fs::write(dir.path().join("clone.md"), "---\nclone: owner/repo\n---\n```markmason\napp cloned\n```\n```expected\nyes\n```\n```markmason\napp echo $REPO_ROOT\n```\n").unwrap();
    let path = std::env::join_paths(
        [dir.path().to_owned(), helper().parent().unwrap().to_owned()]
            .into_iter()
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = command(dir.path())
        .env("PATH", path)
        .arg("--verbose")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    let text = text(&output);
    let cloned = text
        .lines()
        .find(|line| line.contains("markmason-repo-") && !line.contains("PASS"))
        .unwrap();
    assert!(
        !Path::new(cloned).exists(),
        "cloned fixture was not cleaned up: {cloned}"
    );
}

#[test]
fn empty_authentication_environment_values_use_nonempty_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    for (key, expected) in [
        ("XDG_CONFIG_HOME", home.join("gh")),
        ("HOME", home.join(".config/gh")),
        ("USERPROFILE", home.join(".config/gh")),
        #[cfg(windows)]
        ("APPDATA", home.join("GitHub CLI")),
    ] {
        fs::write(
            dir.path().join("config.md"),
            format!(
                "```markmason\napp gh-config\n```\n```expected\n{}\n```\n",
                expected.display()
            ),
        )
        .unwrap();
        let output = command(dir.path())
            .env("GH_CONFIG_DIR", "")
            .env("XDG_CONFIG_HOME", "")
            .env("APPDATA", "")
            .env("HOME", "")
            .env("USERPROFILE", "")
            .env(key, &home)
            .output()
            .unwrap();
        assert!(output.status.success(), "{key}: {}", text(&output));
    }
}

#[test]
fn listing_needs_neither_authentication_nor_a_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("list.md"),
        "---\nclone: owner/repo\nsetup: nonexistent\n---\n```markmason\napp echo listed\n```\n",
    )
    .unwrap();
    let output = command(dir.path())
        .args(["--list", "--cwd"])
        .arg(dir.path().join("nonexistent"))
        .env_remove("GH_CONFIG_DIR")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("APPDATA")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("app echo listed"));
}

#[test]
fn filesystem_root_is_a_valid_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dunce::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let root: PathBuf = cwd
        .components()
        .take_while(|part| !matches!(part, std::path::Component::Normal(_)))
        .collect();
    fs::write(
        dir.path().join("root.md"),
        "```markmason\napp root \"$REPO_ROOT\" \"$PROJECT_NAME\"\n```\n",
    )
    .unwrap();
    let output = command(dir.path()).arg("--cwd").arg(root).output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
}

#[cfg(windows)]
#[test]
fn mixed_case_environment_cannot_override_fixture_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let caller_home = dir.path().join("caller-home");
    fs::create_dir(&caller_home).unwrap();
    let path = std::env::join_paths(
        std::iter::once(helper().parent().unwrap().to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let overrides = [
        ("test_home", caller_home.as_os_str().to_owned()),
        ("xdg_config_home", caller_home.as_os_str().to_owned()),
        ("appdata", caller_home.as_os_str().to_owned()),
        ("localappdata", caller_home.as_os_str().to_owned()),
        ("gh_config_dir", caller_home.as_os_str().to_owned()),
        ("no_color", "0".into()),
        ("force_color", "1".into()),
        ("git_terminal_prompt", "1".into()),
        ("gh_prompt_disabled", "0".into()),
    ];
    for home_env in ["TEST_HOME", "test_home"] {
        fs::write(
            dir.path().join("mixed.md"),
            format!(
                "```markmason\napp env TEST_HOME XDG_CONFIG_HOME APPDATA LOCALAPPDATA GH_CONFIG_DIR NO_COLOR FORCE_COLOR GIT_TERMINAL_PROMPT GH_PROMPT_DISABLED\n```\n```expected\n${home_env}\n$XDG_CONFIG_HOME\n$XDG_CONFIG_HOME\n$XDG_CONFIG_HOME\n{}\n1\n0\n0\n1\n```\n",
                caller_home.display()
            ),
        ).unwrap();
        let inherited = std::env::vars_os().filter(|(key, _)| {
            !overrides
                .iter()
                .any(|(name, _)| key.to_string_lossy().eq_ignore_ascii_case(name))
        });
        let output = command_with_home(dir.path(), home_env)
            .env_clear()
            .envs(inherited)
            .envs(overrides.iter().map(|(name, value)| (*name, value)))
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", text(&output));
    }
    assert_eq!(fs::read_dir(caller_home).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn setup_resolves_executables_from_a_relative_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let specs = dir.path().join("specs");
    let work = dir.path().join("work");
    fs::create_dir(&specs).unwrap();
    fs::create_dir(&work).unwrap();
    fs::hard_link(helper(), work.join("tool.exe")).unwrap();
    for program in ["tool.exe", "tool"] {
        fs::write(
            specs.join("setup.md"),
            format!("---\nsetup: {program}\n---\n```markmason\napp state\n```\n```expected\nready\n```\n"),
        )
        .unwrap();
        let output = command(&specs)
            .current_dir(dir.path())
            .args(["--cwd", "work"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{program}: {}", text(&output));
    }
}

#[cfg(windows)]
#[test]
fn relative_path_entries_resolve_from_the_command_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let specs = dir.path().join("specs");
    let work = dir.path().join("work");
    fs::create_dir(&specs).unwrap();
    fs::create_dir_all(work.join("bin")).unwrap();
    fs::create_dir(dir.path().join("bin")).unwrap();
    fs::hard_link(helper(), work.join("bin/tool.exe")).unwrap();
    fs::hard_link(helper(), dir.path().join("bin/tool.exe")).unwrap();
    let expected = dunce::canonicalize(work.join("bin/tool.exe")).unwrap();
    fs::write(
        specs.join("path.md"),
        format!(
            "---\nsetup: tool.exe setup-location\n---\n```markmason\ntool.exe state\n```\n```expected\n{0}\n```\n```markmason\ntool.exe where\n```\n```expected\n{0}\n```\n```markmason\ntool.exe interactive-location\n```\n```pty\nmatch: {0}\nsend:\nmatch: Ready?\nsend: yes{{enter}}\nmatch: Ready?\nsend: no{{enter}}\n```\n```expected\ndone no\n```\n",
            expected.display()
        ),
    ).unwrap();
    let output = command(&specs)
        .current_dir(dir.path())
        .args(["--cwd", "work"])
        .env("PATH", "bin")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
}

#[cfg(windows)]
#[test]
fn invalid_executables_are_startup_errors_not_expected_exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("invalid.exe");
    fs::write(&program, b"not a Windows executable").unwrap();
    for pty in [false, true] {
        fs::write(
            dir.path().join("invalid.md"),
            format!(
                "```markmason (exit 1)\n\"{}\"\n```\n{}",
                program.display(),
                if pty { "```pty\n```\n" } else { "" }
            ),
        )
        .unwrap();
        let output = command(dir.path()).output().unwrap();
        assert!(!output.status.success(), "{}", text(&output));
        assert!(
            text(&output).contains("Failed to start command"),
            "{}",
            text(&output)
        );
    }
}

#[cfg(windows)]
#[test]
fn successful_jobs_keep_agents_alive_until_the_markdown_file_finishes() {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject},
    };
    for pty in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let started = dir.path().join("started");
        fs::write(
            dir.path().join("agent.md"),
            format!(
                "```markmason\napp {} \"{}\"\n```\n{}\n```markmason\napp await-agent\n```\n```expected\nready\n```\n",
                if pty { "background-pty" } else { "background" },
                started.display(),
                if pty {
                    "```pty\nmatch: Ready?\nsend: yes{enter}\n```\n"
                } else {
                    ""
                }
            ),
        )
        .unwrap();
        let output = command(dir.path()).output().unwrap();
        assert!(output.status.success(), "{}", text(&output));
        let info = fs::read_to_string(started).unwrap();
        let info: Vec<_> = info.lines().collect();
        let pid = info[1].parse().unwrap();
        let handle = unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(ERROR_INVALID_PARAMETER as i32)
            );
        } else {
            let process = unsafe { OwnedHandle::from_raw_handle(handle) };
            assert_eq!(
                unsafe { WaitForSingleObject(process.as_raw_handle(), 1000) },
                WAIT_OBJECT_0,
                "Background agent survived file cleanup"
            );
        }
        for path in &info[2..] {
            assert!(
                !Path::new(path).exists(),
                "Fixture {path} was not cleaned up"
            );
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_jobs_clean_up_descendants_even_after_the_command_exits() {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject},
    };
    for mode in ["drain", "writer", "success", "pty"] {
        let dir = tempfile::tempdir().unwrap();
        let started = dir.path().join("started");
        let helper_mode = match mode {
            "success" => "writer",
            "pty" => "tree",
            _ => mode,
        };
        let extra = match mode {
            "writer" => format!("```stdin\n{}\n```\n", "x".repeat(256 * 1024)),
            "pty" => "```pty\nmatch: before timeout\nsend:\n```\n".to_owned(),
            _ => String::new(),
        };
        fs::write(
            dir.path().join("descendants.md"),
            format!(
                "---\ntimeout: 1\n---\n```markmason\napp {helper_mode} \"{}\"\n```\n{extra}",
                started.display()
            ),
        )
        .unwrap();
        let mut runner = command(dir.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !started.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !started.exists() {
            runner.kill().unwrap();
            panic!(
                "Descendant helper did not start: {}",
                text(&runner.wait_with_output().unwrap())
            );
        }
        let info = fs::read_to_string(started).unwrap();
        let info: Vec<_> = info.lines().collect();
        let pid = info[1].parse().unwrap();
        let handle = unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, pid) };
        let descendant = if handle.is_null() {
            let error = std::io::Error::last_os_error();
            assert_eq!(mode, "success", "Failed to open descendant {pid}: {error}");
            assert_eq!(error.raw_os_error(), Some(ERROR_INVALID_PARAMETER as i32));
            None
        } else {
            Some(unsafe { OwnedHandle::from_raw_handle(handle) })
        };
        if mode != "success" {
            assert_eq!(
                unsafe { WaitForSingleObject(descendant.as_ref().unwrap().as_raw_handle(), 0) },
                WAIT_TIMEOUT,
                "Descendant was not running before timeout"
            );
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while runner.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if runner.try_wait().unwrap().is_none() {
            runner.kill().unwrap();
            panic!(
                "Job cleanup exceeded deadline: {}",
                text(&runner.wait_with_output().unwrap())
            );
        }
        let output = runner.wait_with_output().unwrap();
        if mode == "success" {
            assert!(output.status.success(), "{}", text(&output));
        } else {
            assert!(!output.status.success(), "{mode}: {}", text(&output));
            let expected = match mode {
                "drain" => "Timed out draining command output",
                "writer" => "Failed waiting for stdin writer",
                "pty" => "PTY command timed out",
                _ => unreachable!(),
            };
            assert!(
                text(&output).contains(expected),
                "{mode}: {}",
                text(&output)
            );
        }
        if let Some(descendant) = descendant {
            assert_eq!(
                unsafe { WaitForSingleObject(descendant.as_raw_handle(), 1000) },
                WAIT_OBJECT_0,
                "{mode}: descendant {pid} survived cleanup"
            );
        }
        for path in &info[2..] {
            assert!(
                !Path::new(path).exists(),
                "{mode}: fixture {path} was not cleaned up"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn interrupts_reap_commands_stop_descendants_and_clean_up_fixtures() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        for mode in ["command", "setup", "pty", "drain", "writer"] {
            let dir = tempfile::tempdir().unwrap();
            let started = dir.path().join("started");
            let later = dir.path().join("later");
            let tree = format!(
                "app {} \"{}\"",
                if ["drain", "writer"].contains(&mode) {
                    mode
                } else {
                    "tree"
                },
                started.display()
            );
            let markdown = match mode {
                "setup" => format!(
                    "---\nsetup: '{tree}'\n---\n```markmason\napp mark \"{}\"\n```\n",
                    later.display()
                ),
                "pty" => format!(
                    "```markmason\n{tree}\n```\n```pty\nmatch: never\nsend: y\n```\n```markmason\napp mark \"{}\"\n```\n",
                    later.display()
                ),
                "writer" => format!(
                    "```markmason\n{tree}\n```\n```stdin\n{}\n```\n```markmason\napp mark \"{}\"\n```\n",
                    "x".repeat(256 * 1024),
                    later.display()
                ),
                _ => format!(
                    "```markmason\n{tree}\n```\n```markmason\napp mark \"{}\"\n```\n",
                    later.display()
                ),
            };
            fs::write(dir.path().join("a.md"), markdown).unwrap();
            fs::write(
                dir.path().join("b.md"),
                format!("```markmason\napp mark \"{}\"\n```\n", later.display()),
            )
            .unwrap();
            let mut runner = command(dir.path())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !started.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }

            if !started.exists() {
                runner.kill().unwrap();
                panic!(
                    "Helper did not start: {}",
                    text(&runner.wait_with_output().unwrap())
                );
            }
            let info = fs::read_to_string(&started).unwrap();
            let info: Vec<_> = info.lines().collect();
            if ["drain", "writer"].contains(&mode) {
                let pid: i32 = info[0].parse().unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert_eq!(
                    unsafe { libc::kill(pid, 0) },
                    -1,
                    "Command did not exit before cancellation"
                );
            }
            assert_eq!(unsafe { libc::kill(runner.id() as i32, signal) }, 0);
            let deadline = Instant::now() + Duration::from_secs(5);
            while runner.try_wait().unwrap().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if runner.try_wait().unwrap().is_none() {
                runner.kill().unwrap();
                panic!(
                    "Runner did not cancel: {}",
                    text(&runner.wait_with_output().unwrap())
                );
            }
            let output = runner.wait_with_output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(128 + signal),
                "{mode}: {}",
                text(&output)
            );
            for pid in &info[..2] {
                let pid: i32 = pid.parse().unwrap();
                let stopped = unsafe { libc::kill(pid, 0) } == -1;
                #[cfg(target_os = "linux")]
                let stopped = stopped
                    || fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                        stat.split(") ")
                            .nth(1)
                            .is_some_and(|state| state.starts_with('Z'))
                    });
                assert!(stopped, "{mode}: process {pid} survived cancellation");
            }
            for path in &info[2..] {
                assert!(
                    !Path::new(path).exists(),
                    "{mode}: fixture {path} was not cleaned up"
                );
            }
            assert!(
                !later.exists(),
                "{mode}: execution continued after cancellation"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn oversized_pty_input_cannot_block_timeout_or_cancellation() {
    for signal in [None, Some(libc::SIGINT), Some(libc::SIGTERM)] {
        let dir = tempfile::tempdir().unwrap();
        let started = dir.path().join("started");
        fs::write(
            dir.path().join("blocked.md"),
            format!("---\ntimeout: {}\n---\n```markmason\napp nonreading \"{}\"\n```\n```pty\nmatch: Ready?\nsend: {}\n```\n",
                if signal.is_some() { 30 } else { 1 },
                started.display(), "x".repeat(2 * 1024 * 1024)),
        ).unwrap();
        let mut runner = command(dir.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !started.exists() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !started.exists() {
            runner.kill().unwrap();
            panic!(
                "Nonreading helper did not start: {}",
                text(&runner.wait_with_output().unwrap())
            );
        }
        if let Some(signal) = signal {
            // The scripted send starts 200ms after the prompt is matched.
            std::thread::sleep(Duration::from_millis(400));
            assert_eq!(unsafe { libc::kill(runner.id() as i32, signal) }, 0);
        }
        while runner.try_wait().unwrap().is_none() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if runner.try_wait().unwrap().is_none() {
            runner.kill().unwrap();
            panic!(
                "PTY input blocked deadline/cancellation: {}",
                text(&runner.wait_with_output().unwrap())
            );
        }
        let output = runner.wait_with_output().unwrap();
        assert!(!output.status.success(), "{}", text(&output));
        if let Some(signal) = signal {
            assert_eq!(
                output.status.code(),
                Some(128 + signal),
                "{}",
                text(&output)
            );
        } else {
            assert!(
                text(&output).contains("PTY command timed out"),
                "{}",
                text(&output)
            );
        }
        let info = fs::read_to_string(started).unwrap();
        let mut info = info.lines();
        let pid: i32 = info.next().unwrap().parse().unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "PTY child was not reaped"
        );
        for path in info {
            assert!(
                !Path::new(path).exists(),
                "Fixture {path} was not cleaned up"
            );
        }
    }
}
