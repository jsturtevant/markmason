use crate::{cancellation, matching, spec};
use anstream::{AutoStream, ColorChoice};
use anyhow::{Context, Result, bail};
use dialoguer::{
    FuzzySelect,
    console::{Term, truncate_str},
    theme::ColorfulTheme,
};
use std::{
    io::{self, BufRead, IsTerminal, Write},
    ops::Range,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[1;32m";
const RED: &str = "\x1b[1;31m";
const YELLOW: &str = "\x1b[1;33m";
const RULE: &str = "------------------------------------------------------------";

pub fn pick_file(files: &[&str]) -> Result<Option<usize>> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "The walkthrough file picker needs an interactive terminal; \
             select a file with --file-pattern REGEX"
        );
    }
    cancellation::check()?;
    let term = Term::stderr();
    term.write_line("Use arrow keys; type to search; Enter selects; Esc cancels.")?;
    let labels: Vec<_> = files
        .iter()
        .map(|file| file.escape_debug().to_string())
        .collect();
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose a Markdown walkthrough")
        .items(&labels)
        .default(0)
        .interact_on_opt(&term);
    term.show_cursor()
        .context("Failed to restore the terminal cursor")?;
    cancellation::check()?;
    selection.context("Failed to select a walkthrough file")
}

pub enum Status<'a> {
    Progress,
    Passed,
    Failed(&'a anyhow::Error),
    ExecutionError(&'a anyhow::Error),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Command,
    Block,
    File,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Running,
    Progress,
    Passed,
    Failed,
    ExecutionError,
}

struct Frame {
    source: String,
    ranges: Vec<Range<usize>>,
    position: usize,
    expected: Option<String>,
    expected_exit: i32,
    contains: bool,
    actual: String,
    exit: Option<i32>,
    phase: Phase,
    error: Option<String>,
}

impl Frame {
    fn new(case: &spec::Case, commands: &[String], expected: Option<&str>) -> Result<Self> {
        if commands.len() != case.command_ranges.len() {
            bail!("Walkthrough command count does not match parsed source ranges");
        }
        Ok(Self {
            source: case.source.clone(),
            ranges: case.command_ranges.clone(),
            position: 0,
            expected: expected.map(str::to_owned),
            expected_exit: case.exit_code,
            contains: case.contains || case.pty.is_some(),
            actual: String::new(),
            exit: None,
            phase: Phase::Ready,
            error: None,
        })
    }

    fn code(&self) -> (Vec<String>, usize) {
        let active = &self.ranges[self.position];
        let mut offset = 0;
        let mut focus = 0;
        let mut lines = Vec::new();
        for (index, raw) in self.source.split_inclusive('\n').enumerate() {
            let line = raw.trim_end_matches(['\r', '\n']);
            let end = offset + line.len();
            let selected = active.start <= end && active.end > offset;
            let marker = if selected {
                if active.start >= offset {
                    focus = index;
                }
                ">"
            } else {
                " "
            };
            let state = if selected {
                match self.phase {
                    Phase::Ready => "next",
                    Phase::Running => "running",
                    Phase::Failed | Phase::ExecutionError => "failed",
                    _ => "done",
                }
            } else {
                ""
            };
            lines.push(format!(
                " {marker} {}{}",
                plain(line).replace('\t', "    "),
                if state.is_empty() {
                    String::new()
                } else {
                    format!("  [{state}]")
                }
            ));
            offset += raw.len();
        }
        (lines, focus)
    }
}

pub struct Session {
    output: AutoStream<io::Stdout>,
    input: Receiver<io::Result<Option<String>>>,
    blocks: usize,
    block: usize,
    file: String,
    title: Option<String>,
    frame: Option<Frame>,
    mode: Mode,
    pause_before_block: bool,
    screen: bool,
    screen_active: bool,
    stopped: bool,
}

impl Session {
    pub fn new(blocks: usize) -> Result<Self> {
        let (send, input) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("walkthrough-input".into())
            .spawn(move || {
                let stdin = io::stdin();
                let mut stdin = stdin.lock();
                loop {
                    let mut line = String::new();
                    let result = stdin
                        .read_line(&mut line)
                        .map(|count| if count == 0 { None } else { Some(line) });
                    let finished = !matches!(&result, Ok(Some(_)));
                    if send.send(result).is_err() || finished {
                        break;
                    }
                }
            })
            .context("Failed to start walkthrough input reader")?;
        let mut session = Self {
            output: AutoStream::new(io::stdout(), ColorChoice::Auto),
            input,
            blocks,
            block: 0,
            file: String::new(),
            title: None,
            frame: None,
            mode: Mode::Command,
            pause_before_block: false,
            screen: io::stdout().is_terminal()
                && std::env::var_os("TERM").is_none_or(|term| term != "dumb"),
            screen_active: false,
            stopped: false,
        };
        writeln!(
            session.output,
            "\n{CYAN}MARKMASON WALKTHROUGH{RESET}\n\
             Inspect each Markdown block and collect its real output.\n"
        )?;
        Ok(session)
    }

    fn read_line(&mut self) -> Result<Option<String>> {
        loop {
            cancellation::check()?;
            match self.input.recv_timeout(Duration::from_millis(50)) {
                Ok(line) => return line.context("Failed reading walkthrough input"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => return Err(error).context("Walkthrough input reader stopped"),
            }
        }
    }

    pub fn begin_block(&mut self, file: &str, title: Option<&str>, block: usize) -> Result<()> {
        self.pause_before_block =
            self.mode == Mode::Block || (self.mode == Mode::File && self.file != file);
        if self.pause_before_block {
            self.mode = Mode::Command;
        }
        self.file = file.to_owned();
        self.title = title.map(str::to_owned);
        self.block = block;
        self.frame = None;
        Ok(())
    }

    pub fn prepare(
        &mut self,
        case: &spec::Case,
        commands: &[String],
        expected: Option<&str>,
    ) -> Result<()> {
        let result = (|| {
            self.frame = Some(Frame::new(case, commands, expected)?);
            if self.pause_before_block {
                self.render()?;
                self.pause(false, false)?;
            }
            Ok(())
        })();
        self.stop_on_error(result)
    }

    pub fn preview(&mut self, position: usize) -> Result<()> {
        let result = (|| {
            let frame = self.frame.as_mut().context("Missing walkthrough block")?;
            frame.position = position - 1;
            frame.phase = Phase::Running;
            self.render()
        })();
        self.stop_on_error(result)
    }

    pub fn inspect(
        &mut self,
        actual: &str,
        exit: Option<i32>,
        status: Status<'_>,
        stop_file: bool,
    ) -> Result<()> {
        let result = (|| {
            let complete = !matches!(status, Status::Progress);
            let frame = self.frame.as_mut().context("Missing walkthrough block")?;
            frame.actual = actual.to_owned();
            frame.exit = exit;
            frame.error = None;
            frame.phase = match status {
                Status::Progress => Phase::Progress,
                Status::Passed => Phase::Passed,
                Status::Failed(error) | Status::ExecutionError(error) => {
                    frame.error = Some(format!("{error:#}"));
                    self.mode = Mode::Command;
                    if matches!(status, Status::ExecutionError(_)) {
                        Phase::ExecutionError
                    } else {
                        Phase::Failed
                    }
                }
            };
            self.render()?;
            self.pause(complete, stop_file)
        })();
        self.stop_on_error(result)
    }

    fn render(&mut self) -> Result<()> {
        self.output.flush()?;
        let mut size = if self.screen {
            Some(Term::stdout().size())
        } else {
            None
        };
        let frame = self.frame.as_ref().context("Missing walkthrough block")?;
        let (code, focus) = frame.code();
        let expected = panel(
            frame
                .expected
                .as_deref()
                .unwrap_or("(output is not asserted)"),
        );
        let actual = panel(if frame.actual.is_empty() {
            "(empty)"
        } else {
            &frame.actual
        });
        let details = if matches!(frame.phase, Phase::Failed | Phase::ExecutionError) {
            panel(frame.error.as_deref().context("Missing failure details")?)
        } else {
            Vec::new()
        };
        // Reserve space for controls, a retry hint, echoed input, and the cursor.
        let fixed_rows = 10
            + usize::from(self.title.as_ref().is_some_and(|title| !title.is_empty()))
            + usize::from(frame.exit.is_some() && frame.phase != Phase::Running)
            + if size.is_some() {
                details.len().min(5)
            } else {
                details.len()
            }
            + usize::from(self.mode != Mode::Command)
            + if self.mode == Mode::Command && frame.phase != Phase::Running {
                4
            } else {
                0
            };
        if size.is_some_and(|(rows, columns)| usize::from(rows) < fixed_rows + 3 || columns < 60) {
            size = None;
        }
        let details = window(&details, if size.is_some() { 5 } else { usize::MAX }, 0);
        let body = size.map_or(usize::MAX, |(rows, _)| {
            usize::from(rows).saturating_sub(fixed_rows).max(3)
        });
        let code_budget = code.len().min((body / 2).max(1));
        let expected_budget = expected.len().min(((body - code_budget) / 2).max(1));
        let actual_budget = body.saturating_sub(code_budget + expected_budget).max(1);
        let scope = if frame.ranges.len() > 1 {
            "whole block; "
        } else {
            ""
        };
        let mode = if frame.contains {
            "ordered lines"
        } else {
            "exact"
        };
        let actual_label = match frame.phase {
            Phase::Ready | Phase::Running | Phase::Progress => "ACTUAL OUTPUT SO FAR",
            Phase::ExecutionError => "OUTPUT CAPTURED BEFORE ERROR",
            _ => "ACTUAL OUTPUT",
        };
        let mut lines = vec![
            format!("{CYAN}{RULE}{RESET}"),
            format!(
                "{BOLD}{}{RESET}  |  Block {} of {}",
                plain(&self.file),
                self.block,
                self.blocks
            ),
        ];
        if let Some(title) = self.title.as_deref().filter(|title| !title.is_empty()) {
            lines.push(format!("{BOLD}## {}{RESET}", plain(title)));
        }
        lines.push(format!(
            "{BOLD}COMMAND {} OF {}{RESET}",
            frame.position + 1,
            frame.ranges.len()
        ));
        lines.push(format!(
            "{CYAN}```markmason{}{RESET}",
            if frame.expected_exit == 0 {
                String::new()
            } else {
                format!(" (exit {})", frame.expected_exit)
            }
        ));
        lines.extend(window(&code, code_budget, focus));
        lines.push(format!("{CYAN}```{RESET}"));
        lines.push(format!(
            "{BOLD}EXPECTED OUTPUT{RESET} ({scope}{mode}, exit {})",
            frame.expected_exit
        ));
        lines.extend(window(&expected, expected_budget, 0));
        lines.push(format!("{BOLD}{actual_label}{RESET}"));
        lines.extend(window(
            &actual,
            actual_budget,
            actual.len().saturating_sub(1),
        ));
        lines.push(String::new());
        if let Some(exit) = frame.exit.filter(|_| frame.phase != Phase::Running) {
            lines.push(format!("  Command exited {exit}"));
        }
        match frame.phase {
            Phase::Ready => lines.push("  Ready. No commands in this block have run.".into()),
            Phase::Running => lines.push("  Running command... (Ctrl+C cancels)".into()),
            Phase::Progress => lines.push(format!(
                "  {YELLOW}IN PROGRESS{RESET}  Assertion checked after the block finishes."
            )),
            Phase::Passed => {
                lines.push(format!("  {GREEN}PASS{RESET}  Block matched expectations."))
            }
            Phase::Failed | Phase::ExecutionError => {
                lines.push(format!("  {RED}FAIL{RESET}"));
                lines.extend(details);
            }
        }
        if self.mode != Mode::Command {
            lines.push(format!(
                "  Running to {}; Ctrl+C cancels.",
                if self.mode == Mode::Block {
                    "next block"
                } else {
                    "end of file"
                }
            ));
        }
        if size.is_some() {
            if !self.screen_active {
                io::stdout().write_all(b"\x1b[?1049h")?;
                self.screen_active = true;
            }
            io::stdout().write_all(b"\x1b[H\x1b[2J")?;
        } else {
            self.restore_screen()?;
        }
        for line in lines {
            if let Some((_, columns)) = size {
                let line = line.replace('\t', "    ");
                writeln!(
                    self.output,
                    "{}",
                    truncate_str(&line, usize::from(columns).saturating_sub(1).max(1), "...")
                )?;
            } else {
                writeln!(self.output, "{line}")?;
            }
        }
        self.output.flush()?;
        Ok(())
    }

    fn pause(&mut self, complete: bool, stop_file: bool) -> Result<()> {
        if self.mode != Mode::Command {
            return Ok(());
        }
        let action = if stop_file {
            "Stop this file"
        } else if self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.phase == Phase::Ready)
        {
            "Run first command"
        } else if complete && self.block == self.blocks {
            "Finish walkthrough"
        } else if complete {
            "Next block"
        } else {
            "Next command"
        };
        loop {
            writeln!(
                self.output,
                "{CYAN}Enter{RESET}: {action}  |  {CYAN}[q + Enter]{RESET} Quit"
            )?;
            writeln!(
                self.output,
                "{CYAN}[b + Enter]{RESET} Next block  |  {CYAN}[f + Enter]{RESET} Finish"
            )?;
            self.output.flush()?;
            match self.read_line()?.as_deref().map(str::trim) {
                Some("") => return Ok(()),
                Some("b" | "B") => {
                    self.mode = Mode::Block;
                    return Ok(());
                }
                Some("f" | "F") => {
                    self.mode = Mode::File;
                    return Ok(());
                }
                Some("q" | "Q" | "quit") => {
                    self.stopped = true;
                    return Ok(());
                }
                None => {
                    self.stopped = true;
                    writeln!(self.output, "Input closed. Stopping walkthrough.")?;
                    return Ok(());
                }
                _ => {
                    self.render()?;
                    writeln!(
                        self.output,
                        "Use Enter, b then Enter, f then Enter, or q then Enter."
                    )?;
                }
            }
        }
    }

    fn restore_screen(&mut self) -> Result<()> {
        if self.screen_active {
            self.output.flush()?;
            io::stdout().write_all(b"\x1b[?1049l")?;
            io::stdout().flush()?;
            self.screen_active = false;
        }
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn suspend(&mut self) -> Result<()> {
        let result = self.restore_screen();
        self.stop_on_error(result)
    }

    fn stop_on_error(&mut self, result: Result<()>) -> Result<()> {
        if let Err(error) = &result {
            self.stopped = true;
            if let Err(restore_error) = self.restore_screen() {
                eprintln!("Cannot restore walkthrough screen: {restore_error:#}");
            }
            eprintln!("Walkthrough error: {error:#}");
        }
        result
    }

    pub fn finish(&mut self, passed: usize, failed: usize) -> Result<()> {
        self.restore_screen()?;
        let label = if self.stopped {
            "Walkthrough stopped"
        } else {
            "Walkthrough finished"
        };
        writeln!(
            self.output,
            "\n{CYAN}{RULE}{RESET}\n{BOLD}{label}{RESET}\nPassed blocks: {passed}\nFailed blocks: {failed}"
        )?;
        self.output.flush()?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.restore_screen() {
            eprintln!("Failed to restore walkthrough terminal: {error:#}");
        }
    }
}

fn window(lines: &[String], budget: usize, focus: usize) -> Vec<String> {
    if lines.len() <= budget {
        return lines.to_vec();
    }
    if budget <= 2 {
        let mut shown = vec![lines[focus.min(lines.len() - 1)].clone()];
        if budget == 2 {
            shown.push(format!("  ... {} other lines", lines.len() - 1));
        }
        return shown;
    }
    let count = budget.saturating_sub(2).max(1);
    let start = focus.saturating_sub(count / 2).min(lines.len() - count);
    let end = start + count;
    let mut shown = Vec::new();
    if start > 0 {
        shown.push(format!("  ... {start} lines above"));
    }
    shown.extend_from_slice(&lines[start..end]);
    if end < lines.len() {
        shown.push(format!("  ... {} lines below", lines.len() - end));
    }
    shown
}

fn panel(text: &str) -> Vec<String> {
    plain(text)
        .lines()
        .map(|line| format!("  | {line}"))
        .collect()
}

fn plain(text: &str) -> String {
    matching::strip_ansi(text)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .flat_map(|ch| {
            if ch.is_control() && !matches!(ch, '\n' | '\t') {
                ch.escape_default().collect::<Vec<_>>()
            } else {
                vec![ch]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Frame, Phase, plain, window};

    #[test]
    fn marker_tracks_multiline_commands_without_losing_comments_or_indentation() {
        let parsed = crate::spec::parse(
            "---\nshell: bash\n---\n## Loop\n```markmason\n# Preview: VALUE=one\nVALUE=one\n\nfor value in one two; do\n  printf '%s\\n' \"$value\"\ndone\n```\n",
        ).unwrap();
        let case = &parsed.cases[0];
        let commands = crate::shell::commands(&case.command).unwrap();
        let mut frame = Frame::new(case, &commands, None).unwrap();
        frame.position = 1;
        frame.phase = Phase::Running;
        let (code, focus) = frame.code();
        assert_eq!(focus, 3);
        assert_eq!(code[0], "   # Preview: VALUE=one");
        assert_eq!(code[3], " > for value in one two; do  [running]");
        assert_eq!(code[4], " >   printf '%s\\n' \"$value\"  [running]");
        assert_eq!(code[5], " > done  [running]");
        frame.position = 0;
        let (code, focus) = frame.code();
        assert_eq!(focus, 1);
        assert_eq!(code[0], "   # Preview: VALUE=one");
        assert_eq!(code[1], " > VALUE=one  [running]");
    }

    #[test]
    fn output_keeps_text_readable_without_running_terminal_controls() {
        assert_eq!(
            plain("\x1b[2J\x1b[31mred\x1b[0m\r\nnext\rline\tvalue\x07"),
            "red\nnext\nline\tvalue\\u{7}"
        );
        assert!(!plain("\x1b[1:2mtext").contains('\x1b'));
    }

    #[test]
    fn clipped_panes_keep_the_active_line_and_report_hidden_lines() {
        let lines: Vec<_> = (0..100).map(|i| format!("line {i}")).collect();
        let shown = window(&lines, 8, 50);
        assert!(shown.contains(&"line 50".into()));
        assert!(shown.len() <= 8);
        assert!(shown[0].contains("lines above"));
        assert!(shown.last().unwrap().contains("lines below"));
        for budget in 1..=8 {
            for focus in [0, 50, 99] {
                let shown = window(&lines, budget, focus);
                assert!(shown.len() <= budget);
                assert!(shown.contains(&format!("line {focus}")));
            }
        }
    }
}
