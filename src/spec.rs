use anyhow::{Context, Result, bail};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use regex::Regex;
use serde_yaml::Value;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Spec {
    pub inputs: Vec<Input>,
    pub shell: Option<Shell>,
    pub clone: Option<String>,
    pub setup: Option<String>,
    pub tags: Vec<String>,
    pub timeout: Option<Duration>,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone)]
pub struct Input {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub title: Option<String>,
    pub description: String,
    pub source: String,
    pub command_ranges: Vec<std::ops::Range<usize>>,
    pub command: String,
    pub expected: Option<String>,
    pub exit_code: i32,
    pub always: bool,
    pub matching: crate::matching::Mode,
    pub stdin: Option<String>,
    pub pty: Option<Vec<Step>>,
}

impl Case {
    pub fn output_mode(&self) -> crate::matching::Mode {
        if self.pty.is_some() && self.matching == crate::matching::Mode::Exact {
            crate::matching::Mode::Contains
        } else {
            self.matching
        }
    }
}

#[derive(Debug, Clone)]
pub struct Step {
    pub text: String,
    pub input: String,
    pub raw: bool,
}

pub fn replace_keys(text: &str, keys: &[(&str, &str)]) -> String {
    keys.iter().fold(text.to_owned(), |text, (key, value)| {
        Regex::new(&format!("(?i){}", regex::escape(key)))
            .unwrap()
            .replace_all(&text, regex::NoExpand(value))
            .into_owned()
    })
}

pub fn raw_keys(text: &str) -> String {
    let text = replace_keys(
        text,
        &[
            ("{hide-cursor}", "\x1b[?25l"),
            ("{show-cursor}", "\x1b[?25h"),
            ("{clear-screen}", "\x1b[2J"),
            ("{alt-screen}", "\x1b[?1049h"),
        ],
    );
    Regex::new(r"\\x([0-9a-fA-F]{2})")
        .unwrap()
        .replace_all(&text, |caps: &regex::Captures<'_>| {
            char::from(u8::from_str_radix(&caps[1], 16).unwrap()).to_string()
        })
        .into_owned()
}

pub fn input_keys(text: &str) -> String {
    replace_keys(
        text,
        &[
            ("{enter}", "\r"),
            ("{up}", "\x1b[A"),
            ("{down}", "\x1b[B"),
            ("{space}", " "),
            ("{backspace}", "\x7f"),
            ("{escape}", "\x1b"),
            ("{ctrl-c}", "\x03"),
        ],
    )
}

fn parse_steps(lines: &[&str]) -> Result<Vec<Step>> {
    let mut steps = Vec::new();
    let mut pending = None;
    for line in lines {
        let line = line.trim();
        if let Some(text) = line.strip_prefix("match-raw:") {
            pending = Some((raw_keys(text.trim()), true));
        } else if let Some(text) = line.strip_prefix("match:") {
            pending = Some((text.trim().to_owned(), false));
        } else if let Some(input) = line.strip_prefix("send:") {
            let (text, raw) = pending
                .take()
                .context("PTY parse error: send: without preceding match: or match-raw:")?;
            steps.push(Step {
                text,
                input: input.trim().to_owned(),
                raw,
            });
        }
    }
    if pending.is_some() {
        bail!("PTY parse error: trailing match: or match-raw: without a following send:");
    }
    Ok(steps)
}

pub fn parse(content: &str) -> Result<Spec> {
    parse_with_shell(content, None)
}

pub fn parse_with_shell(content: &str, shell: Option<Shell>) -> Result<Spec> {
    let mut spec = Spec {
        shell,
        ..Spec::default()
    };
    let frontmatter = Regex::new(r"\A---\r?\n([\s\S]*?)\r?\n---\r?\n").unwrap();
    let body = if let Some(caps) = frontmatter.captures(content) {
        let value: Value = serde_yaml::from_str(&caps[1]).context("Invalid YAML frontmatter")?;
        if !value.is_null() {
            let map = value
                .as_mapping()
                .context("Frontmatter must be a YAML object")?;
            if let Some(inputs) = map.get(Value::from("inputs")) {
                let names = Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap();
                for (name, prompt) in inputs
                    .as_mapping()
                    .context("Frontmatter inputs must map environment names to prompt strings")?
                {
                    let name = name
                        .as_str()
                        .filter(|name| names.is_match(name))
                        .context("Input names must be portable environment variable names")?;
                    let prompt = prompt
                        .as_str()
                        .filter(|prompt| !prompt.trim().is_empty() && !prompt.contains('\0'))
                        .context("Each input must have a nonempty prompt string")?;
                    spec.inputs.push(Input {
                        name: name.to_owned(),
                        prompt: prompt.to_owned(),
                    });
                }
            }
            if let Some(value) = map
                .get(Value::from("shell"))
                .filter(|value| !value.is_null())
            {
                if value.as_str() != Some("bash") {
                    bail!("Frontmatter shell must be bash");
                }
                spec.shell = Some(Shell::Bash);
            }
            for (key, target) in [("clone", &mut spec.clone), ("setup", &mut spec.setup)] {
                if let Some(value) = map.get(Value::from(key)).filter(|v| !v.is_null()) {
                    *target = Some(
                        value
                            .as_str()
                            .with_context(|| format!("Frontmatter {key} must be a string"))?
                            .to_owned(),
                    );
                }
            }
            if spec.setup.as_ref().is_some_and(|setup| {
                split_command(setup)
                    .first()
                    .is_none_or(|arg| arg.is_empty())
            }) {
                bail!("Frontmatter setup must be a nonempty command");
            }
            if let Some(tags) = map.get(Value::from("tags")) {
                for tag in tags
                    .as_sequence()
                    .context("Frontmatter tags must be an array of strings")?
                {
                    let tag = tag
                        .as_str()
                        .context("Frontmatter tags must be an array of strings")?
                        .trim()
                        .to_lowercase();
                    if tag.is_empty() {
                        bail!("Frontmatter tags must not contain empty values");
                    }
                    if !spec.tags.contains(&tag) {
                        spec.tags.push(tag);
                    }
                }
            }
            if let Some(value) = map.get(Value::from("timeout")).filter(|v| !v.is_null()) {
                let seconds = value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    .context("Invalid timeout: expected positive seconds")?;
                if !seconds.is_finite() || seconds <= 0.0 {
                    bail!("Invalid timeout: expected positive seconds");
                }
                spec.timeout = Some(
                    Duration::try_from_secs_f64(seconds)
                        .context("Invalid timeout: out of range")?,
                );
            }
        }
        &content[caps.get(0).unwrap().end()..]
    } else {
        content
    };
    let command_info =
        Regex::new(r"^markmason(?:\s+(?P<always>always))?(?:\s+\(exit\s+(?P<exit>\d+)\))?$")
            .unwrap();
    let language_info = Regex::new(r"^[A-Za-z0-9_+#.-]+$").unwrap();
    let mut current = None;
    let mut title = None;
    let mut description = String::new();
    let mut description_start = 0;
    let mut events = Parser::new(body).into_offset_iter();
    while let Some((event, range)) = events.next() {
        if matches!(event, Event::Start(Tag::Heading { .. })) {
            current = None;
            description.clear();
            description_start = range.end;
            let mut text = String::new();
            for (event, _) in events.by_ref() {
                match event {
                    Event::Text(part) | Event::Code(part) => text.push_str(&part),
                    Event::SoftBreak | Event::HardBreak => text.push(' '),
                    Event::End(TagEnd::Heading(_)) => break,
                    _ => {}
                }
            }
            title = Some(text);
            continue;
        }
        let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = event else {
            continue;
        };
        let mut parts = info.split_whitespace();
        let first = parts.next().unwrap_or("");
        let kind = if parts.next() == Some("markmason") {
            "markmason"
        } else {
            first
        };
        let recognized = matches!(
            kind,
            "markmason"
                | "expected"
                | "expected-contains"
                | "expected-substring"
                | "expected-regex"
                | "stdin"
                | "pty"
        );
        let prose = body[description_start..range.start].trim();
        if !prose.is_empty() {
            if !description.is_empty() {
                description.push_str("\n\n");
            }
            description.push_str(prose);
        }
        description_start = range.end;
        let mut block = String::new();
        let mut content_end = body[range.clone()]
            .find('\n')
            .map_or(range.end, |offset| range.start + offset + 1);
        for (event, text_range) in events.by_ref() {
            match event {
                Event::Text(text) => {
                    block.push_str(&text);
                    content_end = text_range.end;
                }
                Event::End(TagEnd::CodeBlock) => break,
                _ => {}
            }
        }
        if recognized {
            // CommonMark accepts unclosed fences; executable specs require a closing fence.
            let marker = body.as_bytes()[range.start];
            if body[content_end..range.end].trim_end().as_bytes().last() != Some(&marker) {
                bail!("Unterminated {kind} block");
            }
            let block = block.strip_suffix('\n').unwrap_or(&block);
            if kind == "markmason" {
                let metadata = if first != "markmason" {
                    if !language_info.is_match(first) {
                        bail!("Invalid markmason language: expected a language identifier");
                    }
                    info[first.len()..].trim_start()
                } else {
                    info.as_ref()
                };
                let caps = command_info.captures(metadata).context(
                    "Invalid markmason fence: expected [LANGUAGE] markmason [always] [(exit N)]",
                )?;
                let command_ranges = if spec.shell.is_some() {
                    crate::shell::command_ranges(block)?
                } else {
                    let mut offset = 0;
                    let mut ranges = Vec::new();
                    for line in block.split_inclusive('\n') {
                        let text = line.trim();
                        if !text.is_empty() {
                            let start = offset + line.len() - line.trim_start().len();
                            ranges.push(start..start + text.len());
                        }
                        offset += line.len();
                    }
                    if ranges.is_empty()
                        || ranges.iter().any(|range| {
                            split_command(&block[range.clone()])
                                .first()
                                .is_none_or(|arg| arg.is_empty())
                        })
                    {
                        bail!("A markmason block must contain nonempty commands");
                    }
                    ranges
                };
                spec.cases.push(Case {
                    title: title.clone(),
                    description: description.clone(),
                    source: block.to_owned(),
                    command: if spec.shell.is_some() {
                        block.to_owned()
                    } else {
                        command_ranges
                            .iter()
                            .map(|range| &block[range.clone()])
                            .collect::<Vec<_>>()
                            .join("\n")
                    },
                    command_ranges,
                    exit_code: caps
                        .name("exit")
                        .map_or(Ok(0), |m| m.as_str().parse())
                        .context(
                            "Invalid markmason exit code: expected a nonnegative 32-bit integer",
                        )?,
                    always: caps.name("always").is_some(),
                    expected: None,
                    matching: crate::matching::Mode::Exact,
                    stdin: None,
                    pty: None,
                });
                current = Some(spec.cases.len() - 1);
            } else if let Some(index) = current {
                let case = &mut spec.cases[index];
                match info.as_ref() {
                    "expected" | "expected-contains" | "expected-substring" | "expected-regex" => {
                        case.matching = match info.as_ref() {
                            "expected-contains" => crate::matching::Mode::Contains,
                            "expected-substring" => crate::matching::Mode::Substring,
                            "expected-regex" => {
                                Regex::new(block).context("Invalid expected-regex pattern")?;
                                crate::matching::Mode::Regex
                            }
                            _ => crate::matching::Mode::Exact,
                        };
                        case.expected = Some(block.to_owned());
                    }
                    "stdin" => case.stdin = Some(format!("{block}\n")),
                    "pty" => case.pty = Some(parse_steps(&block.lines().collect::<Vec<_>>())?),
                    _ => bail!("Invalid {kind} fence: unexpected metadata"),
                }
            }
        }
    }
    Ok(spec)
}

// Deliberately not shell parsing: backslashes are literal, and empty quoted arguments survive.
pub fn split_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut in_token = false;
    for ch in command.trim().chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
            in_token = true;
        } else if ch.is_whitespace() {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(ch);
            in_token = true;
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

pub fn split_pipeline(command: &str) -> Vec<String> {
    let mut stages = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in command.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            current.push(ch);
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
        } else if ch == '|' {
            if !current.trim().is_empty() {
                stages.push(current.trim().to_owned());
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        stages.push(current.trim().to_owned());
    }
    stages
}
