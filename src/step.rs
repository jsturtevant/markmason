use crate::{cancellation, matching, spec};
use anstream::{AutoStream, ColorChoice};
use anyhow::{Context, Result, bail};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind,
    },
    terminal,
};
use dialoguer::{
    FuzzySelect,
    console::{Term, measure_text_width},
    theme::ColorfulTheme,
};
use std::{
    collections::VecDeque,
    io::{self, BufRead, IsTerminal, Write},
    ops::Range,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandState {
    Pending,
    Running,
    Done,
    Failed,
}

struct Frame {
    source: String,
    description: String,
    ranges: Vec<Range<usize>>,
    states: Vec<CommandState>,
    position: usize,
    expected: Option<String>,
    expected_exit: i32,
    contains: bool,
    actual: String,
    exit: Option<i32>,
    phase: Phase,
    error: Option<String>,
    stop_file: bool,
}

impl Frame {
    fn new(case: &spec::Case, commands: &[String], expected: Option<&str>) -> Result<Self> {
        if commands.len() != case.command_ranges.len() {
            bail!("Walkthrough command count does not match parsed source ranges");
        }
        Ok(Self {
            source: case.source.clone(),
            description: case.description.clone(),
            ranges: case.command_ranges.clone(),
            states: vec![CommandState::Pending; commands.len()],
            position: 0,
            expected: expected.map(str::to_owned),
            expected_exit: case.exit_code,
            contains: case.contains || case.pty.is_some(),
            actual: String::new(),
            exit: None,
            phase: Phase::Ready,
            error: None,
            stop_file: false,
        })
    }

    fn next(&self) -> Option<usize> {
        match self.phase {
            Phase::Ready | Phase::Progress => self
                .states
                .iter()
                .position(|state| *state == CommandState::Pending),
            Phase::Running => Some(self.position),
            _ => None,
        }
    }

    fn code(&self) -> Vec<String> {
        let next = self.next();
        let mut offset = 0;
        let mut lines = Vec::new();
        for raw in self.source.split_inclusive('\n') {
            let line = raw.trim_end_matches(['\r', '\n']);
            let end = offset + line.len();
            let overlaps = |range: &Range<usize>| range.start <= end && range.end > offset;
            let preferred = next.unwrap_or(self.position);
            let command = self
                .ranges
                .get(preferred)
                .filter(|range| overlaps(range))
                .map(|_| preferred)
                .or_else(|| self.ranges.iter().position(overlaps));
            let marker = if command.is_some() && command == next {
                ">"
            } else {
                " "
            };
            let state = match command.map(|index| self.states[index]) {
                Some(CommandState::Done) => "[done]",
                Some(CommandState::Running) => "[running]",
                Some(CommandState::Failed) => "[failed]",
                Some(CommandState::Pending) if command == next => "[next]",
                _ => "",
            };
            let label = format!("{state:<9}");
            let label = if state == "[done]" {
                format!("{GREEN}{label}{RESET}")
            } else {
                label
            };
            lines.push(format!(
                " {marker} {label} {}",
                plain(line).replace('\t', "    "),
            ));
            offset += raw.len();
        }
        lines
    }
}

pub struct Session {
    output: AutoStream<io::Stdout>,
    input: Option<Receiver<io::Result<Option<String>>>>,
    pending_keys: VecDeque<Event>,
    blocks: usize,
    block: usize,
    file: String,
    title: Option<String>,
    frame: Option<Frame>,
    mode: Mode,
    pause_before_block: bool,
    screen: bool,
    screen_active: bool,
    scroll: usize,
    page_rows: usize,
    document_rows: usize,
    input_text: String,
    hint: Option<String>,
    stopped: bool,
}

impl Session {
    pub fn new(blocks: usize) -> Result<Self> {
        let screen = io::stdin().is_terminal()
            && io::stdout().is_terminal()
            && std::env::var_os("TERM").is_none_or(|term| term != "dumb");
        let input = if screen {
            None
        } else {
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
            Some(input)
        };
        let mut session = Self {
            output: AutoStream::new(io::stdout(), ColorChoice::Auto),
            input,
            pending_keys: VecDeque::new(),
            blocks,
            block: 0,
            file: String::new(),
            title: None,
            frame: None,
            mode: Mode::Command,
            pause_before_block: false,
            screen,
            screen_active: false,
            scroll: 0,
            page_rows: 1,
            document_rows: 0,
            input_text: String::new(),
            hint: None,
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
        if self.input.is_none() {
            return self.read_keys();
        }
        loop {
            cancellation::check()?;
            match self
                .input
                .as_ref()
                .context("Missing walkthrough input reader")?
                .recv_timeout(Duration::from_millis(50))
            {
                Ok(line) => return line.context("Failed reading walkthrough input"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => return Err(error).context("Walkthrough input reader stopped"),
            }
        }
    }

    pub fn prompt_input(&mut self, file: &str, input: &spec::Input) -> Result<Option<String>> {
        self.restore_screen()?;
        let _raw = self.screen.then(RawInput::new).transpose()?;
        let newline = if self.screen { "\r\n" } else { "\n" };
        write!(
            self.output,
            "{BOLD}{}{RESET}{newline}{}{newline}",
            plain(file),
            plain(&input.prompt).replace('\n', newline)
        )?;
        loop {
            write!(
                self.output,
                "{CYAN}Input {}{RESET} (required; q + Enter cancels):{newline}",
                input.name
            )?;
            self.output.flush()?;
            let value = self.read_line()?;
            let value = value
                .as_deref()
                .map(|value| value.trim_end_matches(['\r', '\n']));
            match value {
                None | Some("q" | "Q" | "quit") => {
                    self.stopped = true;
                    return Ok(None);
                }
                Some(value) if value.trim().is_empty() || value.contains('\0') => {
                    write!(
                        self.output,
                        "Enter a nonempty value without NUL characters.{newline}"
                    )?;
                }
                Some(value) => return Ok(Some(value.to_owned())),
            }
        }
    }

    fn read_keys(&mut self) -> Result<Option<String>> {
        let in_view = self.screen_active;
        let _raw = RawInput::new()?;
        self.input_text.clear();
        loop {
            cancellation::check()?;
            let event = if let Some(event) = self.pending_keys.pop_front() {
                event
            } else {
                if !event::poll(Duration::from_millis(50))? {
                    continue;
                }
                event::read().context("Failed reading walkthrough key")?
            };
            check_key_interrupt(&event)?;
            let movement = match event {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !self.screen_active {
                            write!(self.output, "\r\n")?;
                            self.output.flush()?;
                        }
                        return Ok(if self.input_text.is_empty() {
                            None
                        } else {
                            Some(std::mem::take(&mut self.input_text))
                        });
                    }
                    KeyCode::Enter | KeyCode::Char('j' | 'm')
                        if key.code == KeyCode::Enter
                            || key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        if !self.screen_active {
                            write!(self.output, "\r\n")?;
                            self.output.flush()?;
                        }
                        return Ok(Some(std::mem::take(&mut self.input_text)));
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input_text.clear();
                        self.echo_input()?;
                        None
                    }
                    KeyCode::Char(ch)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        self.input_text.push(ch);
                        self.echo_input()?;
                        None
                    }
                    KeyCode::Backspace => {
                        if let Some((start, _)) = self.input_text.grapheme_indices(true).next_back()
                        {
                            self.input_text.truncate(start);
                        }
                        self.echo_input()?;
                        None
                    }
                    KeyCode::Tab => {
                        self.input_text.push('\t');
                        self.echo_input()?;
                        None
                    }
                    KeyCode::Up => Some(Scroll::Up(1)),
                    KeyCode::Down => Some(Scroll::Down(1)),
                    KeyCode::PageUp => Some(Scroll::Up(self.page_rows)),
                    KeyCode::PageDown => Some(Scroll::Down(self.page_rows)),
                    KeyCode::Home => Some(Scroll::Home),
                    KeyCode::End => Some(Scroll::End),
                    _ => None,
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => Some(Scroll::Up(3)),
                    MouseEventKind::ScrollDown => Some(Scroll::Down(3)),
                    _ => None,
                },
                Event::Resize(_, _) if in_view => {
                    self.render()?;
                    None
                }
                _ => None,
            };
            if let Some(movement) = movement.filter(|_| self.screen_active) {
                self.scroll = match movement {
                    Scroll::Up(rows) => self.scroll.saturating_sub(rows),
                    Scroll::Down(rows) => self.scroll.saturating_add(rows),
                    Scroll::Home => 0,
                    Scroll::End => self.document_rows,
                };
                self.render()?;
            }
        }
    }

    fn collect_keys(&mut self) -> Result<()> {
        // Ctrl+C can be buffered with the key that leaves a raw-mode pause.
        cancellation::check()?;
        if self.screen {
            while event::poll(Duration::ZERO)? {
                let event = event::read().context("Failed reading queued walkthrough key")?;
                check_key_interrupt(&event)?;
                self.pending_keys.push_back(event);
            }
        }
        Ok(())
    }

    fn echo_input(&mut self) -> Result<()> {
        let columns = usize::from(Term::stdout().size().1)
            .saturating_sub(3)
            .max(1);
        let text = plain(&self.input_text).replace('\t', "    ");
        let mut tail = String::new();
        for part in text.graphemes(true).rev() {
            if measure_text_width(part) + measure_text_width(&tail) > columns {
                break;
            }
            tail.insert_str(0, part);
        }
        self.output.flush()?;
        io::stdout().write_all(b"\r\x1b[2K")?;
        write!(
            self.output,
            "{}{tail}",
            if self.screen_active { "> " } else { "" }
        )?;
        self.output.flush()?;
        Ok(())
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
        self.scroll = 0;
        self.hint = None;
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
                self.pause()?;
            }
            Ok(())
        })();
        self.stop_on_error(result)
    }

    pub fn preview(&mut self, position: usize) -> Result<()> {
        let result = (|| {
            self.collect_keys()?;
            let frame = self.frame.as_mut().context("Missing walkthrough block")?;
            frame.position = position - 1;
            *frame
                .states
                .get_mut(frame.position)
                .context("Invalid walkthrough command position")? = CommandState::Running;
            frame.phase = Phase::Running;
            self.scroll = 0;
            self.hint = None;
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
            let frame = self.frame.as_mut().context("Missing walkthrough block")?;
            frame.actual = actual.to_owned();
            frame.exit = exit;
            frame.error = None;
            frame.stop_file = stop_file;
            frame.states[frame.position] = if matches!(status, Status::ExecutionError(_))
                || (exit.is_some_and(|code| code != 0) && !matches!(status, Status::Passed))
            {
                CommandState::Failed
            } else {
                CommandState::Done
            };
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
            self.pause()
        })();
        self.stop_on_error(result)
    }

    fn render(&mut self) -> Result<()> {
        self.output.flush()?;
        let frame = self.frame.as_ref().context("Missing walkthrough block")?;
        let code = frame.code();
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
        if !frame.description.is_empty() {
            lines.push(String::new());
            lines.extend(plain(&frame.description).lines().map(str::to_owned));
            lines.push(String::new());
        }
        lines.push(format!(
            "{BOLD}COMMAND {} OF {}{RESET}",
            frame.next().unwrap_or(frame.position) + 1,
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
        lines.extend(code);
        lines.push(format!("{CYAN}```{RESET}"));
        lines.push(format!(
            "{BOLD}EXPECTED OUTPUT{RESET} ({scope}{mode}, exit {})",
            frame.expected_exit
        ));
        lines.extend(expected);
        lines.push(format!("{BOLD}{actual_label}{RESET}"));
        lines.extend(actual);
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
        let (rows, columns) = Term::stdout().size();
        let newline = if self.screen && terminal::is_raw_mode_enabled()? {
            "\r\n"
        } else {
            "\n"
        };
        if !self.screen || rows < 8 || columns < 30 {
            self.restore_screen()?;
            for line in lines {
                write!(self.output, "{line}{newline}")?;
            }
            self.output.flush()?;
            return Ok(());
        }
        if !self.screen_active {
            self.screen_active = true;
            io::stdout().write_all(b"\x1b[?1049h")?;
            crossterm::execute!(io::stdout(), EnableMouseCapture)?;
        }
        self.page_rows = usize::from(rows) - 4;
        let lines = wrap(&lines, usize::from(columns) - 1);
        self.document_rows = lines.len();
        self.scroll = self.scroll.min(lines.len().saturating_sub(self.page_rows));
        io::stdout().write_all(b"\x1b[H\x1b[2J")?;
        for index in self.scroll..self.scroll + self.page_rows {
            write!(
                self.output,
                "{}{newline}",
                lines.get(index).map(String::as_str).unwrap_or("")
            )?;
        }
        let waiting = self.mode == Mode::Command && frame.phase != Phase::Running;
        let action = self.action();
        let controls = if waiting {
            if columns >= 60 {
                format!(
                    "Enter: {action}  |  [q + Enter] Quit\n[b + Enter] Next block  |  [f + Enter] Finish"
                )
            } else {
                format!("Enter: {action}\nb: Block  f: Finish  q: Quit")
            }
        } else {
            "Running; Ctrl+C cancels.\n".to_owned()
        };
        for line in controls.lines().chain(std::iter::repeat("")).take(2) {
            write!(self.output, "{line}{newline}")?;
        }
        let position = format!(
            "Rows {}-{}/{}",
            self.scroll + 1,
            (self.scroll + self.page_rows).min(self.document_rows),
            self.document_rows
        );
        let help = self
            .hint
            .as_deref()
            .unwrap_or("Scroll: arrows, PgUp/PgDn, Home/End");
        let joined = format!("{help}  |  {position}");
        let status = if measure_text_width(&joined) < usize::from(columns) {
            joined
        } else if self.hint.is_some() {
            "Use Enter/b/f/q.".to_owned()
        } else {
            position
        };
        write!(self.output, "{status}{newline}> ")?;
        self.echo_input()?;
        self.output.flush()?;
        Ok(())
    }

    fn action(&self) -> &str {
        let frame = self.frame.as_ref();
        let complete = frame.is_some_and(|frame| {
            matches!(
                frame.phase,
                Phase::Passed | Phase::Failed | Phase::ExecutionError
            )
        });
        if frame.is_some_and(|frame| frame.stop_file) {
            "Stop this file"
        } else if frame.is_some_and(|frame| frame.phase == Phase::Ready) {
            "Run first command"
        } else if complete && self.block == self.blocks {
            "Finish walkthrough"
        } else if complete {
            "Next block"
        } else {
            "Next command"
        }
    }

    fn pause(&mut self) -> Result<()> {
        let _raw = (self.screen && self.mode == Mode::Command)
            .then(RawInput::new)
            .transpose()?;
        self.render()?;
        if self.mode != Mode::Command {
            return Ok(());
        }
        let newline = if self.screen { "\r\n" } else { "\n" };
        loop {
            if !self.screen_active {
                let action = self.action().to_owned();
                write!(
                    self.output,
                    "{CYAN}Enter{RESET}: {action}  |  {CYAN}[q + Enter]{RESET} Quit{newline}"
                )?;
                write!(
                    self.output,
                    "{CYAN}[b + Enter]{RESET} Next block  |  {CYAN}[f + Enter]{RESET} Finish{newline}"
                )?;
                self.output.flush()?;
            }
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
                    write!(self.output, "Input closed. Stopping walkthrough.{newline}")?;
                    return Ok(());
                }
                _ => {
                    self.hint =
                        Some("Use Enter, b then Enter, f then Enter, or q then Enter.".to_owned());
                    self.render()?;
                    if !self.screen_active {
                        write!(
                            self.output,
                            "Use Enter, b then Enter, f then Enter, or q then Enter.{newline}"
                        )?;
                    }
                }
            }
        }
    }

    fn restore_screen(&mut self) -> Result<()> {
        if self.screen_active {
            self.output.flush()?;
            crossterm::execute!(io::stdout(), DisableMouseCapture)?;
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

enum Scroll {
    Up(usize),
    Down(usize),
    Home,
    End,
}

fn check_key_interrupt(event: &Event) -> Result<()> {
    if let Event::Key(key) = event
        && key.kind != KeyEventKind::Release
        && key.code == KeyCode::Char('c')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        cancellation::interrupt();
        bail!("Interrupted by keyboard");
    }
    Ok(())
}

struct RawInput {
    restore: bool,
}

impl RawInput {
    fn new() -> Result<Self> {
        let restore = !terminal::is_raw_mode_enabled()?;
        if restore {
            terminal::enable_raw_mode().context("Failed to enable walkthrough keyboard input")?;
        }
        Ok(Self { restore })
    }
}

impl Drop for RawInput {
    fn drop(&mut self) {
        if self.restore
            && let Err(error) = terminal::disable_raw_mode()
        {
            eprintln!("Failed to restore walkthrough keyboard input: {error}");
        }
    }
}

fn wrap(lines: &[String], columns: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    for line in lines {
        let color = [GREEN, RED, YELLOW, CYAN, BOLD]
            .into_iter()
            .find(|color| line.contains(color));
        let text = matching::strip_ansi(line).replace('\t', "    ");
        let mut chunk = String::new();
        let mut width = 0;
        for grapheme in text.graphemes(true) {
            let cells = measure_text_width(grapheme);
            if !chunk.is_empty() && width + cells > columns {
                wrapped.push(colorize(&chunk, color));
                chunk.clear();
                width = 0;
            }
            chunk.push_str(grapheme);
            width += cells;
        }
        wrapped.push(colorize(&chunk, color));
    }
    wrapped
}

fn colorize(text: &str, color: Option<&str>) -> String {
    color.map_or_else(|| text.to_owned(), |color| format!("{color}{text}{RESET}"))
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
    use super::{CommandState, Frame, GREEN, Phase, plain, wrap};

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
        frame.states = vec![CommandState::Done, CommandState::Running];
        let code = frame
            .code()
            .iter()
            .map(|line| plain(line))
            .collect::<Vec<_>>();
        assert_eq!(code[0].trim(), "# Preview: VALUE=one");
        assert!(code[1].trim().starts_with("[done]"));
        assert!(code[3].trim().starts_with("> [running]"));
        assert!(code[4].contains("  printf '%s\\n' \"$value\""));
        assert!(code[5].trim().starts_with("> [running]"));
        frame.position = 0;
        frame.phase = Phase::Progress;
        frame.states = vec![CommandState::Done, CommandState::Pending];
        let code = frame.code();
        assert!(code[1].contains(GREEN));
        assert!(!code[1].contains('>'));
        assert!(plain(&code[3]).trim().starts_with("> [next]"));
        frame.position = 1;
        frame.states[1] = CommandState::Done;
        frame.phase = Phase::Passed;
        assert_eq!(
            frame
                .code()
                .iter()
                .filter(|line| plain(line).contains("[done]"))
                .count(),
            4
        );
        assert!(!frame.code().iter().any(|line| line.contains('>')));
    }

    #[test]
    fn same_line_marker_prioritizes_next_running_and_failed_commands() {
        let parsed = crate::spec::parse(
            "---\nshell: bash\n---\n```markmason\nprintf 'one'; printf 'two'\n```\n",
        )
        .unwrap();
        let case = &parsed.cases[0];
        let commands = crate::shell::commands(&case.command).unwrap();
        let mut frame = Frame::new(case, &commands, None).unwrap();
        for (phase, position, state, prefix) in [
            (Phase::Progress, 0, CommandState::Pending, "> [next]"),
            (Phase::Running, 1, CommandState::Running, "> [running]"),
            (Phase::Failed, 1, CommandState::Failed, "[failed]"),
            (Phase::ExecutionError, 1, CommandState::Failed, "[failed]"),
            (Phase::Passed, 1, CommandState::Done, "[done]"),
        ] {
            frame.phase = phase;
            frame.position = position;
            frame.states = vec![CommandState::Done, state];
            let code = frame.code();
            assert_eq!(code.len(), 1);
            let line = plain(&code[0]);
            assert!(line.trim().starts_with(prefix), "{line}");
            assert!(line.ends_with("printf 'one'; printf 'two'"), "{line}");
            assert_eq!(line.contains('>'), prefix.starts_with('>'), "{line}");
        }
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
    fn wrapped_document_keeps_every_character_and_unicode_grapheme() {
        let text = format!(
            "  {} \u{1f469}\u{200d}\u{1f4bb} caf\u{e9}  ",
            "x".repeat(120)
        );
        let shown = wrap(std::slice::from_ref(&text), 20);
        assert_eq!(shown.concat(), text);
        assert!(
            shown
                .iter()
                .all(|line| dialoguer::console::measure_text_width(line) <= 20)
        );
        assert!(
            shown
                .iter()
                .any(|line| line.contains("\u{1f469}\u{200d}\u{1f4bb}"))
        );
        let lines = (0..100)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>();
        assert_eq!(wrap(&lines, 80), lines);
    }
}
