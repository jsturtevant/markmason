use super::*;

#[test]
fn input_frontmatter_requires_named_plain_text_prompts() -> Result<()> {
    let parsed = spec::parse(
        "---\ninputs:\n  FIRST: First question\n  SECOND: Second question\n---\n```markmason\napp echo ready\n```\n",
    )?;
    assert_eq!(parsed.inputs.len(), 2);
    assert_eq!(parsed.inputs[0].name, "FIRST");
    assert_eq!(parsed.inputs[1].prompt, "Second question");
    for invalid in [
        "[]",
        "null",
        "true",
        "{BAD-NAME: question}",
        "{1BAD: question}",
        "{GOOD: ''}",
        "{GOOD: 123}",
        "{GOOD: {prompt: nested}}",
    ] {
        assert!(
            spec::parse(&format!("---\ninputs: {invalid}\n---\n")).is_err(),
            "{invalid}"
        );
    }
    Ok(())
}

#[test]
fn bash_terminal_boundaries_preserve_following_output_and_wait_for_complete_frames() {
    for (raw, after) in [
        ("output\r\nTOKEN\r\nlater", "later"),
        ("output\r\r\nTOKEN\r\r\nlater", "later"),
        ("output\x1b[2;1HTOKEN\x1b[3;1Hlater", "later"),
        ("TOKEN\r\n\x1b[31mlater", "\x1b[31mlater"),
        ("TOKEN\x1b[0m\r\nlater", "later"),
        ("trace 'TOKEN'\r\nTOKEN\r\nlater", "later"),
        ("TOKEN not a frame\r\nTOKEN\r\nlater", "later"),
    ] {
        let end = shell::boundary_end(raw.as_bytes(), "TOKEN").unwrap();
        assert_eq!(&raw[end..], after);
    }
    assert!(shell::boundary_end(b"output\r\nTOKEN", "TOKEN").is_none());
    assert_eq!(
        shell::boundary_end_at_eof(b"output\r\nTOKEN", "TOKEN"),
        Some(b"output\r\nTOKEN".len())
    );
    assert!(shell::boundary_end(b"output\r\nTOKEN\x1b[3;", "TOKEN").is_none());
    assert!(shell::boundary_end(b"trace TOKEN\r\n", "TOKEN").is_none());
    assert!(shell::boundary_end(b"TOKEN'\r\n", "TOKEN").is_none());
}

#[test]
fn shell_frontmatter_is_explicit_and_inherits_into_markdown_setup() -> Result<()> {
    let source =
        "---\nshell: bash\n---\n```markmason\nif true; then\n  printf 'ready\\n'\nfi\n```\n";
    let parsed = spec::parse(source)?;
    assert_eq!(parsed.shell, Some(spec::Shell::Bash));
    assert!(parsed.cases[0].command.contains("\n  printf"));
    let inherited = spec::parse_with_shell("```markmason\nvalue=shared\n```\n", parsed.shell)?;
    assert_eq!(inherited.shell, Some(spec::Shell::Bash));
    assert!(
        spec::parse("```markmason\napp echo literal\n```\n")?
            .shell
            .is_none()
    );
    for invalid in ["zsh", "pwsh", "[]", "42", "''"] {
        assert!(spec::parse(&format!("---\nshell: {invalid}\n---\n")).is_err());
    }
    Ok(())
}

#[test]
fn bash_commands_keep_complete_shell_constructs_and_heredocs() -> Result<()> {
    for (script, expected) in [
        (
            "# prose\nVALUE=ready\nprintf '%s\\n' \"$VALUE\"\n",
            vec!["VALUE=ready", "printf '%s\\n' \"$VALUE\""],
        ),
        (
            "printf '%s\\n' one \\\n  two | cat\n",
            vec!["printf '%s\\n' one \\\n  two | cat"],
        ),
        (
            "for value in one two; do\n  printf '%s\\n' \"$value\"\ndone\n",
            vec!["for value in one two; do\n  printf '%s\\n' \"$value\"\ndone"],
        ),
        (
            "greet() {\n  printf 'hello\\n'\n}\ngreet\n",
            vec!["greet() {\n  printf 'hello\\n'\n}", "greet"],
        ),
        (
            "cat <<'END'\n  hello\nEND\nprintf 'done\\n'\n",
            vec!["cat <<'END'\n  hello\nEND", "printf 'done\\n'"],
        ),
        (
            "false || printf 'recovered\\n'\n",
            vec!["false || printf 'recovered\\n'"],
        ),
        (
            "printf 'one'; printf 'two'\n",
            vec!["printf 'one'", "printf 'two'"],
        ),
        (
            "sleep 0.1 & # leave it running\nBACKGROUND=$!\nwait \"$BACKGROUND\"\n",
            vec!["sleep 0.1 &", "BACKGROUND=$!", "wait \"$BACKGROUND\""],
        ),
    ] {
        assert_eq!(shell::commands(script)?, expected, "{script}");
    }
    for invalid in [
        "if true; then\n",
        "printf 'unterminated\n",
        "# comment only\n",
    ] {
        assert!(shell::commands(invalid).is_err(), "{invalid}");
    }
    Ok(())
}

#[test]
fn environment_overrides_follow_platform_case_rules() {
    for (inherited, key) in [
        ("test_home", "TEST_HOME"),
        ("\u{e9}2e_home", "\u{c9}2E_HOME"),
    ] {
        let mut env = process::Environment::from([(inherited.into(), "caller".into())]);
        process::set_env(&mut env, key, "fixture");
        assert_eq!(env.get(std::ffi::OsStr::new(key)).unwrap(), "fixture");
        #[cfg(windows)]
        assert_eq!(env.len(), 1);
        #[cfg(not(windows))]
        assert_eq!(env.get(std::ffi::OsStr::new(inherited)).unwrap(), "caller");
    }
}

#[test]
fn frontmatter_and_tags() -> Result<()> {
    let spec = spec::parse(
        "---\r\nclone: owner/repo\r\nsetup: node setup.js\r\ntags: [Dispatch, SLOW, slow]\r\ntimeout: '0.5'\r\nparallel: false\r\n---\r\n```markmason (exit 2)\r\ncmd\r\n```\r\n",
    )?;
    assert_eq!(spec.clone.as_deref(), Some("owner/repo"));
    assert_eq!(spec.setup.as_deref(), Some("node setup.js"));
    assert_eq!(spec.tags, ["dispatch", "slow"]);
    assert_eq!(spec.timeout, Some(Duration::from_millis(500)));
    assert!(!spec.parallel);
    assert_eq!(spec.cases[0].command, "cmd");
    assert_eq!(spec.cases[0].exit_code, 2);
    for invalid in [
        "[]",
        "42",
        "tags: null",
        "tags: slow",
        "tags: [42]",
        "tags: [' ']",
        "timeout: 0",
        "timeout: -1",
        "timeout: .inf",
        "timeout: nope",
        "setup: []",
        "setup: ''",
        "setup: '   '",
        "setup: '\"\"'",
        "parallel: nope",
        "parallel: 1",
    ] {
        assert!(
            spec::parse(&format!("---\n{invalid}\n---\n")).is_err(),
            "{invalid}"
        );
    }
    assert!(spec::parse("---\n\n---\n")?.cases.is_empty());
    assert!(spec::parse("# Title")?.parallel);
    assert!(spec::parse("# Title")?.tags.is_empty());
    Ok(())
}

#[test]
fn setup_commands_resolve_files_without_assuming_an_interpreter() -> Result<()> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("setup script.py"), "")?;
    assert_eq!(
        resolve_setup(
            "python3 -I 'setup script.py' --name 'two words' ''",
            dir.path()
        )?,
        vec![
            "python3".to_owned(),
            "-I".into(),
            dir.path()
                .join("setup script.py")
                .to_string_lossy()
                .into_owned(),
            "--name".into(),
            "two words".into(),
            "".into(),
        ]
    );
    assert_eq!(
        resolve_setup("custom ./missing.py ../shared.py", dir.path())?,
        vec![
            "custom".to_owned(),
            dir.path()
                .join("./missing.py")
                .to_string_lossy()
                .into_owned(),
            dir.path()
                .join("../shared.py")
                .to_string_lossy()
                .into_owned(),
        ]
    );
    assert!(resolve_setup("", dir.path()).is_err());
    assert!(resolve_setup("''", dir.path()).is_err());
    Ok(())
}

#[test]
fn parses_all_blocks_and_smoke_tests() -> Result<()> {
    let spec = spec::parse(
        "```markmason (exit 1)\ncmd a\n```\nprose\n```expected-contains\nerror\n```\n```stdin\ny\n```\n\
         ```markmason\ncmd b\n```\n```pty\nmatch: Create?\nsend: y{enter}\n\nmatch-raw: {hide-cursor}\nsend: q\n```\n\
         ```expected\ndone\n```\n```markmason\ncmd c\n```\nSmoke.\n",
    )?;
    assert_eq!(spec.cases.len(), 3);
    let first = &spec.cases[0];
    assert_eq!(first.command, "cmd a");
    assert_eq!(first.expected.as_deref(), Some("error"));
    assert_eq!(first.stdin.as_deref(), Some("y\n"));
    assert_eq!(first.exit_code, 1);
    assert_eq!(first.matching, matching::Mode::Contains);
    let second = &spec.cases[1];
    let steps = second.pty.as_ref().unwrap();
    assert!(!steps[0].raw);
    assert_eq!(steps[0].input, "y{enter}");
    assert!(steps[1].raw);
    assert_eq!(steps[1].text, "\x1b[?25l");
    assert_eq!(second.expected.as_deref(), Some("done"));
    assert_eq!(second.matching, matching::Mode::Exact);
    assert!(spec.cases[2].expected.is_none());
    for block in ["send: y", "match: prompt", "match-raw: \\x1b[2J"] {
        assert!(spec::parse(&format!("```markmason\ncmd\n```\n```pty\n{block}\n```")).is_err());
    }
    Ok(())
}

#[test]
fn fenced_commands_leave_headings_and_nested_examples_as_documentation() -> Result<()> {
    let spec = spec::parse(
        r#"# Commands are separate from headings

## `legacy-command` (exit 2)

```sh
not-a-test
```

````markdown
```markmason
nested-example
```
````

~~~text
```markmason
another-example
```
~~~

    ```markmason
    indented-example
    ```

## Run a real command

```markmason
real-command
```
````stdin
```markmason
literal-input
```
````
````expected
```markmason
literal-output
```
````
"#,
    )?;
    assert_eq!(spec.cases.len(), 1);
    assert_eq!(spec.cases[0].command, "real-command");
    assert_eq!(
        spec.cases[0].stdin.as_deref(),
        Some("```markmason\nliteral-input\n```\n")
    );
    assert_eq!(
        spec.cases[0].expected.as_deref(),
        Some("```markmason\nliteral-output\n```")
    );
    Ok(())
}

#[test]
fn fenced_cases_bind_blocks_until_the_next_command_or_heading() -> Result<()> {
    let spec = spec::parse(
        "```markmason\nfirst\n```\n```expected\none\n```\n\
         ```markmason (exit 2)\nsecond\n```\n```expected-contains\ntwo\n```\n\
         ## An unrelated section\n```expected\nnot an assertion\n```\n\
         ### A smoke test\n  ~~~~markmason\n\n  third  \n\n  ~~~~~ \n",
    )?;
    assert_eq!(spec.cases.len(), 3);
    assert_eq!(spec.cases[0].expected.as_deref(), Some("one"));
    assert_eq!(spec.cases[1].exit_code, 2);
    assert_eq!(spec.cases[1].expected.as_deref(), Some("two"));
    assert_eq!(spec.cases[1].matching, matching::Mode::Contains);
    assert_eq!(spec.cases[2].command, "third");
    assert!(spec.cases[2].expected.is_none());
    Ok(())
}

#[test]
fn fenced_cases_stop_at_setext_headings() -> Result<()> {
    for underline in ["=", "===", "-", "---", "  === \t", "   ---"] {
        let spec = spec::parse(&format!(
            "```markmason\nfirst\n```\n```expected\noriginal\n```\n\
             Other examples\n{underline}\n\
             ```expected-contains\nmust not overwrite\n```\n\
             ```stdin\nmust not send\n```\n\
             ```pty\nmatch: prompt\nsend: must not send\n```\n\
             ```markmason\nsecond\n```\n```expected\nsecond result\n```\n"
        ))?;
        assert_eq!(spec.cases.len(), 2, "{underline:?}");
        let first = &spec.cases[0];
        assert_eq!(first.expected.as_deref(), Some("original"), "{underline:?}");
        assert_eq!(first.matching, matching::Mode::Exact, "{underline:?}");
        assert!(first.stdin.is_none(), "{underline:?}");
        assert!(first.pty.is_none(), "{underline:?}");
        assert_eq!(spec.cases[1].expected.as_deref(), Some("second result"));
    }
    for divider in ["---", "==="] {
        let spec = spec::parse(&format!(
            "```markmason\ncmd\n```\n\n{divider}\n\n```expected\nstill attached\n```\n"
        ))?;
        assert_eq!(spec.cases[0].expected.as_deref(), Some("still attached"));
    }
    Ok(())
}

#[test]
fn fenced_assertions_survive_lists_and_thematic_breaks() -> Result<()> {
    for prose in [
        "***\n---",
        "- Explanation\n---",
        "1. Explanation\n---",
        "> Explanation\n\n---",
        "<hr>\n\n---",
    ] {
        let spec = spec::parse(&format!(
            "```markmason\ncmd\n```\n\n{prose}\n\n```expected\nmust match\n```\n"
        ))?;
        assert_eq!(spec.cases.len(), 1);
        assert_eq!(
            spec.cases[0].expected.as_deref(),
            Some("must match"),
            "{prose}"
        );
    }
    Ok(())
}

#[test]
fn fenced_literal_examples_in_containers_do_not_run_or_hide_real_tests() -> Result<()> {
    for marker in ["`", "~"] {
        for (opener, prefix) in [
            ("- ", "  "),
            ("+ ", "  "),
            ("* ", "  "),
            ("1. ", "   "),
            ("12) ", "    "),
            ("> ", "> "),
            ("- > ", "  > "),
        ] {
            let spec = spec::parse(&format!(
                "{opener}{marker}{marker}{marker}{marker}markdown\n\
                 {prefix}{marker}{marker}{marker}markmason\n\
                 {prefix}example-command\n\
                 {prefix}{marker}{marker}{marker}\n\
                 {prefix}{marker}{marker}{marker}{marker}\n\n\
                 ~~~markmason\nreal-command\n~~~\n"
            ))?;
            assert_eq!(spec.cases.len(), 1, "{marker}, {opener:?}");
            assert_eq!(
                spec.cases[0].command, "real-command",
                "{marker}, {opener:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn pty_transcripts_preserve_conpty_cursor_encoded_line_starts() {
    let raw = "\x1b[?25lTitle\x1b[9;1HCOMMAND 1 OF 1\r\n\
               echo hello\x1b[12;1HEXPECTED OUTPUT\r\n\
               \x1b[32mhello\x1b[0m\x1b[2;1HWalkthrough cancelled.\x1b[K";
    assert_eq!(
        matching::pty_text(raw),
        "Title\nCOMMAND 1 OF 1\r\necho hello\nEXPECTED OUTPUT\r\nhello\nWalkthrough cancelled."
    );
    assert_eq!(
        matching::strip_ansi(raw),
        "TitleCOMMAND 1 OF 1\r\necho helloEXPECTED OUTPUT\r\nhelloWalkthrough cancelled."
    );
    assert_eq!(
        matching::pty_text("\x1b[3;1fnext\r\nline\x1b[1;5Hsame row"),
        "\nnext\r\nlinesame row"
    );
}

#[test]
fn fenced_commands_in_containers_preserve_their_input_and_assertions() -> Result<()> {
    for (opener, prefix) in [("- ", "  "), ("1. ", "   "), ("> ", "> ")] {
        let spec = spec::parse(&format!(
            "{opener}```markmason\n{prefix}app cat\n{prefix}```\n\
             {prefix}```stdin\n{prefix}if True:\n{prefix}    print(\"ok\")\n{prefix}```\n\
             {prefix}```expected\n{prefix}ok\n{prefix}```"
        ))?;
        assert_eq!(spec.cases.len(), 1, "{opener:?}");
        let case = &spec.cases[0];
        assert_eq!(case.command, "app cat");
        assert_eq!(case.stdin.as_deref(), Some("if True:\n    print(\"ok\")\n"));
        assert_eq!(case.expected.as_deref(), Some("ok"));
    }
    Ok(())
}

#[test]
fn fenced_contents_remove_only_the_opening_indentation() -> Result<()> {
    for marker in ["```", "~~~"] {
        for indent in 0..=3 {
            let pad = " ".repeat(indent);
            let spec = spec::parse(&format!(
                "{pad}{marker}markmason\n{pad}app cat\n{pad}{marker}\n\
                 {pad}{marker}stdin\n\
                 {pad}if True:\n{pad}    print(\"ok\")\n\n{pad}print(\"done\")\n\
                 unindented\n{pad}\tkeep tab\n{pad}{marker}\n\
                 {pad}{marker}expected\n{pad}ok\n{pad}  extra indentation\n{pad}{marker}\n\
                 {pad}{marker}pty\n{pad}match: Ready?\n{pad}send: yes\n{pad}{marker}\n"
            ))?;
            assert_eq!(spec.cases.len(), 1);
            let case = &spec.cases[0];
            assert_eq!(case.command, "app cat");
            assert_eq!(
                case.stdin.as_deref(),
                Some("if True:\n    print(\"ok\")\n\nprint(\"done\")\nunindented\n\tkeep tab\n"),
                "{marker}, indent {indent}"
            );
            assert_eq!(
                case.expected.as_deref(),
                Some("ok\n  extra indentation"),
                "{marker}, indent {indent}"
            );
            let steps = case.pty.as_ref().unwrap();
            assert_eq!(steps[0].text, "Ready?");
            assert_eq!(steps[0].input, "yes");
        }
    }
    let spec = spec::parse(
        "   ```markmason\n   app cat\n   ```\n   ```stdin\n one space\n  two spaces\n    four spaces\n   ```\n",
    )?;
    assert_eq!(
        spec.cases[0].stdin.as_deref(),
        Some("one space\ntwo spaces\n four spaces\n")
    );
    Ok(())
}

#[test]
fn walkthrough_descriptions_preserve_section_prose_but_not_executable_fences() -> Result<()> {
    let spec = spec::parse(
        "## Prepare\nUse **existing** resources and `safe` values.\n\n- Keep this context.\n\
         ```markmason\nfirst\n```\n```expected\none\n```\n\
         Explain the next command.\n```markmason\nsecond\n```\n\
         ## Finish\nClean only owned resources.\n```markmason\nthird\n```\n",
    )?;
    assert_eq!(
        spec.cases[0].description,
        "Use **existing** resources and `safe` values.\n\n- Keep this context."
    );
    assert!(
        spec.cases[1]
            .description
            .ends_with("Explain the next command.")
    );
    assert!(!spec.cases[1].description.contains("```"));
    assert!(!spec.cases[1].description.contains("\none\n"));
    assert_eq!(spec.cases[2].description, "Clean only owned resources.");
    Ok(())
}

#[test]
fn case_titles_follow_markdown_headings_without_changing_commands() -> Result<()> {
    let spec = spec::parse(
        "```markmason\nno-title\n```\n\
         ## **Send** `input`\n```markmason\nfirst\n```\n```markmason\nsecond\n```\n\
         Another *section*\n---\n```markmason\nthird\n```\n",
    )?;
    assert_eq!(spec.cases[0].title, None);
    assert_eq!(spec.cases[1].title.as_deref(), Some("Send input"));
    assert_eq!(spec.cases[2].title.as_deref(), Some("Send input"));
    assert_eq!(spec.cases[3].title.as_deref(), Some("Another section"));
    assert_eq!(spec.cases[3].command, "third");
    Ok(())
}

#[test]
fn invalid_command_fences_report_errors() {
    for (markdown, expected) in [
        ("```markmason\n", "Unterminated markmason block"),
        ("```markmason\ncmd\n~~~\n", "Unterminated markmason block"),
        ("````markmason\ncmd\n```\n", "Unterminated markmason block"),
        (
            "```markmason\ncmd\n``` not a closing fence",
            "Unterminated markmason block",
        ),
        (
            "- ```markmason\n  cmd\n\noutside\n",
            "Unterminated markmason block",
        ),
        (
            "> ~~~markmason\n> cmd\n\noutside\n",
            "Unterminated markmason block",
        ),
        (
            "> ~~~markmason\n> cmd\n>   ",
            "Unterminated markmason block",
        ),
        ("```markmason\n```\n", "must contain nonempty commands"),
        ("```markmason\n  \n```\n", "must contain nonempty commands"),
        (
            "```markmason\n\"\"\n```\n",
            "must contain nonempty commands",
        ),
        (
            "```markmason\nfirst\n\"\"\nsecond\n```\n",
            "must contain nonempty commands",
        ),
        (
            "```markmason (exit -1)\ncmd\n```\n",
            "Invalid markmason fence",
        ),
        (
            "```markmason (exit nope)\ncmd\n```\n",
            "Invalid markmason fence",
        ),
        (
            "```markmason (exit 2147483648)\ncmd\n```\n",
            "Invalid markmason exit code",
        ),
        (
            "```markmason unexpected\ncmd\n```\n",
            "Invalid markmason fence",
        ),
        (
            "```markmason\ncmd\n```\n```expected\noutput",
            "Unterminated expected block",
        ),
        (
            "```markmason\ncmd\n```\n```stdin\ninput",
            "Unterminated stdin block",
        ),
        (
            "```markmason\ncmd\n```\n```pty\nmatch: prompt",
            "Unterminated pty block",
        ),
        (
            "```markmason\ncmd\n```\n```expected extra\noutput\n```",
            "Invalid expected fence",
        ),
    ] {
        let error = spec::parse(markdown).unwrap_err().to_string();
        assert!(error.contains(expected), "{markdown:?}: {error}");
    }
}

#[test]
fn command_blocks_allow_multiple_commands_and_blank_lines() -> Result<()> {
    let spec = spec::parse(
        "## Run several commands\r\n\r\n```markmason (exit 2)\r\n\r\n\
         first \"two words\"\r\n\r\n  second | third  \r\nlast\r\n\r\n```\r\n\
         ```expected\r\ncombined output\r\n```\r\n",
    )?;
    assert_eq!(spec.cases.len(), 1);
    let case = &spec.cases[0];
    assert_eq!(case.command, "first \"two words\"\nsecond | third\nlast");
    assert_eq!(case.exit_code, 2);
    assert_eq!(case.expected.as_deref(), Some("combined output"));
    Ok(())
}

#[test]
fn key_placeholders_and_ansi() {
    assert_eq!(
        spec::raw_keys("{HIDE-CURSOR}{show-cursor}{clear-screen}{alt-screen}\\x1b[2J"),
        "\x1b[?25l\x1b[?25h\x1b[2J\x1b[?1049h\x1b[2J"
    );
    assert_eq!(
        spec::input_keys("a{ENTER}{up}{Down}{space}{backspace}{Escape}{CTRL-C}"),
        "a\r\x1b[A\x1b[B \x7f\x1b\x03"
    );
    assert_eq!(
        matching::strip_ansi("\x1b[31mred\x1b[0m\x1b[?25l\x1b]0;title\x07"),
        "red"
    );
}

#[test]
fn tokenization_matches_original_runner() {
    for (input, expected) in [
        ("rally --help", vec!["rally", "--help"]),
        (
            "grep -c 'Total session time:'",
            vec!["grep", "-c", "Total session time:"],
        ),
        ("echo \"hello world\"", vec!["echo", "hello world"]),
        (
            "grep \"foo bar\" 'baz qux'",
            vec!["grep", "foo bar", "baz qux"],
        ),
        ("  rally  ", vec!["rally"]),
        ("echo \"hello", vec!["echo", "hello"]),
        ("", vec![]),
        ("cmd \"\" --flag", vec!["cmd", "", "--flag"]),
        ("cmd ''", vec!["cmd", ""]),
        (r"cmd C:\some\path", vec!["cmd", r"C:\some\path"]),
    ] {
        assert_eq!(spec::split_command(input), expected, "{input}");
    }
    for (input, expected) in [
        ("echo hello|grep hello", vec!["echo hello", "grep hello"]),
        ("cmd1  |  cmd2", vec!["cmd1", "cmd2"]),
        ("rally --help", vec!["rally --help"]),
        ("| cmd1 || cmd2 |", vec!["cmd1", "cmd2"]),
        ("grep -E 'a|b' file", vec!["grep -E 'a|b' file"]),
        ("echo \"a|b\" | wc -c", vec!["echo \"a|b\"", "wc -c"]),
        ("a|b|c", vec!["a", "b", "c"]),
    ] {
        assert_eq!(spec::split_pipeline(input), expected, "{input}");
    }
}

#[test]
fn normalization_and_wrapping() {
    for (actual, expected) in [
        ("  hello   world  ", "hello world"),
        ("a\n\nb\n\n", "a\nb"),
        (r"path\to\\file", "path/to/file"),
        ("hello\nworld", "hello world"),
        ("hello\nworld", "helloworld"),
        ("a\nb\nc", "a b c"),
        ("hello\nworld", "hello\nworld"),
    ] {
        matching::check(actual, expected, matching::Mode::Exact).unwrap();
    }
}

#[test]
fn exact_and_contains_failures() {
    for (actual, expected, message) in [
        ("actual", "expected", "Line 1 mismatch"),
        ("a\nb", "a", "Extra actual line"),
        ("a", "a\nb", "Missing expected line"),
    ] {
        assert!(
            matching::check(actual, expected, matching::Mode::Exact)
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
    matching::check("noise\na\nnoise\nb", "a\nb", matching::Mode::Contains).unwrap();
    matching::check("", "", matching::Mode::Contains).unwrap();
    for (actual, expected, message) in [
        ("a\nc", "a\nb", "Expected line 2 not found"),
        ("noise\na\ntrailing", "a", "unmatched trailing line"),
        ("b\na", "a\nb", "Expected line 2 not found"),
        ("noise", "", "unmatched trailing line"),
        ("prefix a", "a", "Expected line 1 not found"),
    ] {
        assert!(
            matching::check(actual, expected, matching::Mode::Contains)
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
}

#[test]
fn always_fences_compose_with_languages_exits_and_assertions() -> Result<()> {
    for opening in [
        "markmason always (exit 2)",
        "bash markmason always (exit 2)",
        "csharp markmason always (exit 2)",
    ] {
        let parsed = spec::parse(&format!(
            "```{opening}\napp fail\n```\n```expected-substring\nstderr\n```\n"
        ))?;
        let case = &parsed.cases[0];
        assert!(case.always);
        assert_eq!(case.exit_code, 2);
        assert_eq!(case.matching, matching::Mode::Substring);
        assert_eq!(case.expected.as_deref(), Some("stderr"));
    }
    let parsed = spec::parse(
        "```bash markmason\napp echo ordinary\n```\n```markmason always\napp echo deferred\n```\n",
    )?;
    assert!(!parsed.cases[0].always);
    assert!(parsed.cases[1].always);
    for opening in [
        "markmason (exit 2) always",
        "markmason always always",
        "markmason always (exit -1)",
        "markmason sometimes",
        "bash! markmason always",
    ] {
        assert!(spec::parse(&format!("```{opening}\napp fail\n```\n")).is_err());
    }
    assert!(
        spec::parse("```markmason always\napp echo deferred")
            .unwrap_err()
            .to_string()
            .contains("Unterminated markmason")
    );
    Ok(())
}

#[test]
fn bash_recovery_interrupts_only_before_completion_markers_arrive() {
    assert!(shell::recovery_needs_interrupt(None, None));
    assert!(!shell::recovery_needs_interrupt(Some(0), None));
    assert!(!shell::recovery_needs_interrupt(None, Some(1)));
    assert!(!shell::recovery_needs_interrupt(Some(0), Some(1)));
}

#[test]
fn parses_substring_and_regex_assertions() -> Result<()> {
    for (fence, mode, expected) in [
        ("expected-substring", matching::Mode::Substring, r"a[1]\b"),
        ("expected-regex", matching::Mode::Regex, r"(?m)^id: \d+$"),
    ] {
        let markdown = format!("```markmason\ncmd\n```\n```{fence}\n{expected}\n```\n");
        let parsed = spec::parse(&markdown)?;
        assert_eq!(parsed.cases[0].expected.as_deref(), Some(expected));
        assert_eq!(parsed.cases[0].output_mode(), mode);

        let with_pty = spec::parse(&format!(
            "{markdown}```pty\nmatch: ready\nsend: y{{enter}}\n```\n"
        ))?;
        assert_eq!(with_pty.cases[0].output_mode(), mode);
        assert!(
            spec::parse(&format!("```markmason\ncmd\n```\n```{fence}\n{expected}"))
                .unwrap_err()
                .to_string()
                .contains(&format!("Unterminated {fence} block"))
        );
        assert!(
            spec::parse(&format!(
                "```markmason\ncmd\n```\n```{fence} extra\n{expected}\n```\n"
            ))
            .is_err()
        );
    }
    let error = spec::parse("```markmason\ncmd\n```\n```expected-regex\n[\n```\n").unwrap_err();
    assert!(format!("{error:#}").contains("Invalid expected-regex pattern"));
    assert!(format!("{error:#}").contains("unclosed character class"));
    Ok(())
}

#[test]
fn substring_and_regex_match_literal_multiline_output() {
    for (actual, expected, mode) in [
        (
            "prefix a[1]\\b trailing\nextra",
            r"a[1]\b",
            matching::Mode::Substring,
        ),
        (
            "prefix first\r\nsecond trailing",
            "first\nsecond",
            matching::Mode::Substring,
        ),
        ("id: 42\nextra", r"(?m)^id: \d+$", matching::Mode::Regex),
        (
            "prefix first\nsecond trailing",
            r"(?s)first.*second",
            matching::Mode::Regex,
        ),
        (
            "Saved C:\\work\\report[1].txt (ready)\n",
            r"C:\\work\\report\[1\]\.txt",
            matching::Mode::Regex,
        ),
        (
            "first\r\nsecond\r\n",
            "^first\nsecond\n$",
            matching::Mode::Regex,
        ),
        ("", "", matching::Mode::Substring),
        ("", "", matching::Mode::Regex),
    ] {
        matching::check(actual, expected, mode).unwrap();
    }
    for (actual, expected, mode) in [
        ("prefix a1 trailing", "a[1]", matching::Mode::Substring),
        ("one  two", "one two", matching::Mode::Substring),
        ("path\\to", "path/to", matching::Mode::Substring),
        ("First", "first", matching::Mode::Substring),
        ("id: abc", r"id: \d+", matching::Mode::Regex),
        ("first\nsecond", "first.*second", matching::Mode::Regex),
        ("one  two", "one two", matching::Mode::Regex),
    ] {
        let error = matching::check(actual, expected, mode)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("{expected:?}")), "{error}");
        assert!(error.contains(actual), "{error}");
    }
    assert!(
        matching::check("output", "[", matching::Mode::Regex)
            .unwrap_err()
            .to_string()
            .contains("Invalid expected-regex pattern")
    );
}

#[test]
fn commands_use_path_and_script_resolution_is_preserved() -> Result<()> {
    let dir = Path::new("/specs");
    assert_eq!(
        resolve("rally 'hello world'", dir)?,
        ["rally", "hello world"]
    );
    assert_eq!(
        resolve("node ./helper.js arg", dir)?,
        vec![
            "node".to_owned(),
            dir.join("./helper.js").to_string_lossy().into_owned(),
            "arg".into()
        ]
    );
    assert!(resolve("", dir).is_err());
    Ok(())
}

#[test]
fn filters_and_timeout_validation() -> Result<()> {
    assert_eq!(tags(Some("Dispatch, SLOW,,".into())), ["dispatch", "slow"]);
    assert!(regex(Some("(".into()), "FILE_PATTERN").is_err());
    assert!(regex(Some("".into()), "FILE_PATTERN")?.is_none());
    for invalid in ["0", "-1", "NaN", "inf", "nope"] {
        assert!(positive_seconds(invalid).is_err());
    }
    assert_eq!(positive_seconds("0.1").unwrap(), 0.1);
    assert!(matches!(parallelism("auto").unwrap(), Parallelism::Auto));
    assert!(matches!(parallelism("8").unwrap(), Parallelism::Jobs(8)));
    for invalid in ["0", "-1", "nope", "257"] {
        assert!(parallelism(invalid).is_err(), "{invalid}");
    }
    Ok(())
}
