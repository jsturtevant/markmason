use crate::{cancellation, matching, spec};
use anstream::{AutoStream, ColorChoice};
use anyhow::{Context, Result, bail};
use dialoguer::{FuzzySelect, console::Term, theme::ColorfulTheme};
use std::{
    io::{self, BufRead, IsTerminal, Write},
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

pub struct Session {
    output: AutoStream<io::Stdout>,
    input: Receiver<io::Result<Option<String>>>,
    blocks: usize,
    block: usize,
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
            stopped: false,
        };
        writeln!(
            session.output,
            "\n{CYAN}MARKMASON WALKTHROUGH{RESET}\n\
             Run a command, inspect its output, then continue at your pace.\n"
        )?;
        Ok(session)
    }

    pub fn begin_block(&mut self, file: &str, title: Option<&str>, block: usize) -> Result<()> {
        self.block = block;
        let result = (|| {
            writeln!(
                self.output,
                "{CYAN}{RULE}{RESET}\n{BOLD}{}{RESET}  |  Block {} of {}",
                plain(file),
                self.block,
                self.blocks
            )?;
            if let Some(title) = title.filter(|title| !title.is_empty()) {
                writeln!(self.output, "{BOLD}{}{RESET}", plain(title))?;
            }
            self.output.flush()?;
            Ok(())
        })();
        self.stop_on_error(result)
    }

    pub fn preview(
        &mut self,
        case: &spec::Case,
        command: &str,
        expected: Option<&str>,
        position: usize,
        commands: usize,
    ) -> Result<()> {
        let result = (|| {
            self.command(
                command,
                position,
                commands,
                expected,
                case.exit_code,
                case.contains || case.pty.is_some(),
            )?;
            writeln!(self.output, "\n  Running command...")?;
            self.output.flush()?;
            Ok(())
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
            self.result(actual, exit, status)?;
            self.pause(complete, stop_file)
        })();
        self.stop_on_error(result)
    }

    fn command(
        &mut self,
        command: &str,
        position: usize,
        commands: usize,
        expected: Option<&str>,
        exit: i32,
        contains: bool,
    ) -> Result<()> {
        writeln!(
            self.output,
            "\n{BOLD}COMMAND {position} OF {commands}{RESET}\n  $ {}",
            plain(command)
        )?;
        let scope = if commands > 1 { "whole block; " } else { "" };
        let mode = if contains { "ordered lines" } else { "exact" };
        writeln!(
            self.output,
            "\n{BOLD}EXPECTED OUTPUT{RESET} ({scope}{mode}, exit {exit})"
        )?;
        self.panel(expected.unwrap_or("(output is not asserted)"))?;
        self.output.flush()?;
        Ok(())
    }

    fn result(&mut self, actual: &str, exit: Option<i32>, status: Status<'_>) -> Result<()> {
        let label = match status {
            Status::Progress => "ACTUAL OUTPUT SO FAR",
            Status::ExecutionError(_) => "OUTPUT CAPTURED BEFORE ERROR",
            _ => "ACTUAL OUTPUT",
        };
        writeln!(self.output, "\n{BOLD}{label}{RESET}")?;
        self.panel(if actual.is_empty() { "(empty)" } else { actual })?;
        if let Some(exit) = exit {
            writeln!(self.output, "\n  Command exited {exit}")?;
        }
        match status {
            Status::Progress => writeln!(
                self.output,
                "  {YELLOW}IN PROGRESS{RESET}  Assertion checked after the block finishes."
            )?,
            Status::Passed => writeln!(
                self.output,
                "  {GREEN}PASS{RESET}  Block matched expectations."
            )?,
            Status::Failed(error) | Status::ExecutionError(error) => {
                writeln!(self.output, "  {RED}FAIL{RESET}")?;
                self.panel(&format!("{error:#}"))?;
            }
        }
        writeln!(self.output)?;
        Ok(())
    }

    fn pause(&mut self, complete: bool, stop_file: bool) -> Result<()> {
        let action = if stop_file {
            "Stop this file"
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
                "{CYAN}Enter{RESET}: {action}  |  {CYAN}q + Enter{RESET}: Quit"
            )?;
            self.output.flush()?;
            let line = loop {
                cancellation::check()?;
                match self.input.recv_timeout(Duration::from_millis(50)) {
                    Ok(line) => break line.context("Failed reading walkthrough input")?,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(error) => return Err(error).context("Walkthrough input reader stopped"),
                }
            };
            match line.as_deref().map(str::trim) {
                Some("") => return Ok(()),
                Some("q" | "Q" | "quit") => {
                    self.stopped = true;
                    return Ok(());
                }
                None => {
                    self.stopped = true;
                    writeln!(self.output, "Input closed. Stopping walkthrough.")?;
                    return Ok(());
                }
                _ => writeln!(
                    self.output,
                    "Press Enter to continue, or type q then Enter to quit."
                )?,
            }
        }
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn finish(&mut self, passed: usize, failed: usize) -> Result<()> {
        let label = if self.stopped {
            "Walkthrough stopped"
        } else {
            "Walkthrough finished"
        };
        writeln!(
            self.output,
            "\n{CYAN}{RULE}{RESET}\n{BOLD}{label}{RESET}\n\
             Passed blocks: {passed}\nFailed blocks: {failed}"
        )?;
        self.output.flush()?;
        Ok(())
    }

    fn panel(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            writeln!(self.output, "  | (empty)")?;
        }
        for line in plain(text).lines() {
            writeln!(self.output, "  | {line}")?;
        }
        Ok(())
    }

    fn stop_on_error(&mut self, result: Result<()>) -> Result<()> {
        if let Err(error) = &result {
            self.stopped = true;
            eprintln!("Walkthrough error: {error:#}");
        }
        result
    }
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
    use super::plain;

    #[test]
    fn output_keeps_text_readable_without_running_terminal_controls() {
        assert_eq!(
            plain("\x1b[2J\x1b[31mred\x1b[0m\r\nnext\rline\tvalue\x07"),
            "red\nnext\nline\tvalue\\u{7}"
        );
        assert!(!plain("\x1b[1:2mtext").contains('\x1b'));
    }
}
