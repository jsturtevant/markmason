use anyhow::{Context, Result, bail};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use regex::Regex;
use serde_yaml::Value;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Spec {
    pub clone: Option<String>,
    pub setup: Option<String>,
    pub tags: Vec<String>,
    pub timeout: Option<Duration>,
    pub cases: Vec<Case>,
}

#[derive(Debug)]
pub struct Case {
    pub title: Option<String>,
    pub command: String,
    pub expected: Option<String>,
    pub exit_code: i32,
    pub contains: bool,
    pub stdin: Option<String>,
    pub pty: Option<Vec<Step>>,
}

#[derive(Debug)]
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
    let mut spec = Spec::default();
    let frontmatter = Regex::new(r"\A---\r?\n([\s\S]*?)\r?\n---\r?\n").unwrap();
    let body = if let Some(caps) = frontmatter.captures(content) {
        let value: Value = serde_yaml::from_str(&caps[1]).context("Invalid YAML frontmatter")?;
        if !value.is_null() {
            let map = value
                .as_mapping()
                .context("Frontmatter must be a YAML object")?;
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
    let command_info = Regex::new(r"^markmason(?:\s+\(exit\s+(\d+)\))?$").unwrap();
    let mut current = None;
    let mut title = None;
    let mut events = Parser::new(body).into_offset_iter();
    while let Some((event, range)) = events.next() {
        if matches!(event, Event::Start(Tag::Heading { .. })) {
            current = None;
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
        let kind = info.split_whitespace().next().unwrap_or("");
        let recognized = matches!(
            kind,
            "markmason" | "expected" | "expected-contains" | "stdin" | "pty"
        );
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
                let caps = command_info
                    .captures(&info)
                    .context("Invalid markmason fence: expected markmason or markmason (exit N)")?;
                let commands: Vec<_> = block
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect();
                if commands.is_empty()
                    || commands.iter().any(|command| {
                        split_command(command)
                            .first()
                            .is_none_or(|arg| arg.is_empty())
                    })
                {
                    bail!("A markmason block must contain nonempty commands");
                }
                spec.cases.push(Case {
                    title: title.clone(),
                    command: commands.join("\n"),
                    exit_code: caps.get(1).map_or(Ok(0), |m| m.as_str().parse()).context(
                        "Invalid markmason exit code: expected a nonnegative 32-bit integer",
                    )?,
                    expected: None,
                    contains: false,
                    stdin: None,
                    pty: None,
                });
                current = Some(spec.cases.len() - 1);
            } else if let Some(index) = current {
                let case = &mut spec.cases[index];
                match info.as_ref() {
                    "expected" | "expected-contains" => {
                        case.expected = Some(block.to_owned());
                        case.contains = info.as_ref() == "expected-contains";
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
