use anstream::{AutoStream, ColorChoice, StripStream};
use dialoguer::console::measure_text_width;
use std::{
    io::{self, IsTerminal, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[1;32m";
const RED: &str = "\x1b[1;31m";
const YELLOW: &str = "\x1b[1;33m";
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Clone)]
pub struct Block {
    pub position: usize,
    pub total: usize,
    pub heading: String,
    pub command: String,
    pub deferred: bool,
}

impl Block {
    pub fn kind(&self) -> &'static str {
        if self.deferred { "Always" } else { "Block" }
    }

    pub fn label(&self) -> String {
        format!(
            "{} {}/{} — {}",
            self.kind(),
            self.position,
            self.total,
            self.heading
        )
    }
}

#[derive(Clone)]
pub struct Document {
    pub file: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub failed_headings: Vec<String>,
    pub cancelled: bool,
    pub not_run: bool,
    pub duration: Duration,
}

impl Document {
    pub fn failed(&self) -> bool {
        self.failed > 0 || self.errors > 0
    }

    pub fn succeeded(&self) -> bool {
        !self.failed() && !self.cancelled && !self.not_run
    }
}

pub struct Presenter {
    tty: bool,
    started: Instant,
    output: Destination,
}

enum Destination {
    Live(AutoStream<std::io::Stdout>),
    Buffered(StripStream<Vec<u8>>),
}

impl Write for Destination {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Live(output) => output.write(buffer),
            Self::Buffered(output) => output.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Live(output) => output.flush(),
            Self::Buffered(output) => output.flush(),
        }
    }
}

impl Presenter {
    pub fn new() -> Self {
        let tty = io::stdout().is_terminal()
            && std::env::var_os("TERM").is_none_or(|term| term != "dumb");
        Self {
            tty,
            started: Instant::now(),
            output: Destination::Live(AutoStream::new(io::stdout(), ColorChoice::Auto)),
        }
    }

    pub fn buffered() -> Self {
        Self {
            tty: false,
            started: Instant::now(),
            output: Destination::Buffered(StripStream::new(Vec::new())),
        }
    }

    pub fn into_buffer(self) -> String {
        match self.output {
            Destination::Buffered(output) => {
                String::from_utf8_lossy(&output.into_inner()).into_owned()
            }
            Destination::Live(_) => String::new(),
        }
    }

    pub fn document_start(&mut self, file: &str) -> io::Result<()> {
        if self.tty {
            writeln!(
                self.output,
                "{CYAN}{}{RESET}",
                fit(&format!("▶ {file}"), terminal_width())
            )?;
        } else {
            writeln!(self.output, "RUN  {file}")?;
        }
        self.output.flush()
    }

    pub fn block_start(&mut self, block: &Block) -> io::Result<Progress> {
        if self.tty {
            Progress::new(block)
        } else {
            writeln!(self.output, "  RUN  {}", block.label())?;
            self.output.flush()?;
            Ok(Progress::inactive())
        }
    }

    pub fn block_pass(
        &mut self,
        progress: Progress,
        block: &Block,
        diagnostics: &str,
    ) -> io::Result<()> {
        progress.finish()?;
        self.details(diagnostics)?;
        if !self.tty || block.deferred || !diagnostics.is_empty() {
            let line = format!("  ✓ PASS {}", block.label());
            writeln!(
                self.output,
                "{GREEN}{}{RESET}",
                if self.tty {
                    fit(&line, terminal_width())
                } else {
                    line
                }
            )?;
            self.output.flush()?;
        }
        Ok(())
    }

    pub fn block_fail(
        &mut self,
        progress: Progress,
        file: &str,
        block: &Block,
        error: &anyhow::Error,
        diagnostics: &str,
    ) -> io::Result<()> {
        progress.finish()?;
        self.details(diagnostics)?;
        writeln!(self.output, "  {RED}✗ FAIL{RESET}  {}", block.label())?;
        writeln!(self.output, "    Document: {file}")?;
        for (index, command) in block.command.lines().enumerate() {
            writeln!(
                self.output,
                "{}{command}",
                if index == 0 {
                    "    Command: "
                } else {
                    "             "
                }
            )?;
        }
        for line in format!("{error:#}").lines() {
            writeln!(self.output, "    {line}")?;
        }
        self.output.flush()
    }

    pub fn block_cancel(
        &mut self,
        progress: Progress,
        file: &str,
        block: &Block,
        error: &anyhow::Error,
        diagnostics: &str,
    ) -> io::Result<()> {
        progress.finish()?;
        self.details(diagnostics)?;
        writeln!(
            self.output,
            "  {YELLOW}■ CANCELLED{RESET}  {}",
            block.label()
        )?;
        writeln!(self.output, "    Document: {file}")?;
        for (index, command) in block.command.lines().enumerate() {
            writeln!(
                self.output,
                "{}{command}",
                if index == 0 {
                    "    Command: "
                } else {
                    "             "
                }
            )?;
        }
        for line in format!("{error:#}").lines() {
            writeln!(self.output, "    {line}")?;
        }
        self.output.flush()
    }

    pub fn details(&mut self, diagnostics: &str) -> io::Result<()> {
        if !diagnostics.is_empty() {
            write!(self.output, "{diagnostics}")?;
            self.output.flush()?;
        }
        Ok(())
    }

    pub fn document_finish(&mut self, document: &Document) -> io::Result<()> {
        let elapsed = duration(document.duration);
        if document.failed() {
            if document.failed > 0 {
                let line = format!(
                    "✗ FAIL  {}  {} passed, {} failed  {elapsed}",
                    document.file, document.passed, document.failed
                );
                writeln!(
                    self.output,
                    "{RED}{}{RESET}",
                    if self.tty {
                        fit(&line, terminal_width())
                    } else {
                        line
                    }
                )?;
            } else {
                let line = format!(
                    "✗ FAIL  {}  {}/{} blocks, {} document error(s)  {elapsed}",
                    document.file, document.passed, document.total, document.errors
                );
                writeln!(
                    self.output,
                    "{RED}{}{RESET}",
                    if self.tty {
                        fit(&line, terminal_width())
                    } else {
                        line
                    }
                )?;
            }
        } else if document.cancelled {
            let line = format!(
                "■ CANCELLED  {}  {}/{} blocks  {elapsed}",
                document.file, document.passed, document.total
            );
            writeln!(
                self.output,
                "{YELLOW}{}{RESET}",
                if self.tty {
                    fit(&line, terminal_width())
                } else {
                    line
                }
            )?;
        } else if document.succeeded() {
            let line = format!(
                "✓ PASS  {}  {}/{} blocks  {elapsed}",
                document.file, document.passed, document.total
            );
            writeln!(
                self.output,
                "{GREEN}{}{RESET}",
                if self.tty {
                    fit(&line, terminal_width())
                } else {
                    line
                }
            )?;
        }
        self.output.flush()
    }

    pub fn setup_fail(&mut self, file: &str, error: &anyhow::Error) -> io::Result<()> {
        writeln!(self.output, "  {RED}✗ FAIL{RESET}  Setup")?;
        writeln!(self.output, "    Document: {file}")?;
        for line in format!("{error:#}").lines() {
            writeln!(self.output, "    {line}")?;
        }
        self.output.flush()
    }

    pub fn document_error(
        &mut self,
        file: &str,
        label: &str,
        error: &anyhow::Error,
    ) -> io::Result<()> {
        writeln!(self.output, "  {RED}✗ FAIL{RESET}  {label}")?;
        writeln!(self.output, "    Document: {file}")?;
        for line in format!("{error:#}").lines() {
            writeln!(self.output, "    {line}")?;
        }
        self.output.flush()
    }

    pub fn document_cancel(
        &mut self,
        file: &str,
        label: &str,
        error: &anyhow::Error,
    ) -> io::Result<()> {
        writeln!(self.output, "  {YELLOW}■ CANCELLED{RESET}  {label}")?;
        writeln!(self.output, "    Document: {file}")?;
        for line in format!("{error:#}").lines() {
            writeln!(self.output, "    {line}")?;
        }
        self.output.flush()
    }

    pub fn list_document(&mut self, file: &str, blocks: &[(Block, String)]) -> io::Result<()> {
        writeln!(self.output, "{BOLD}{file}{RESET}")?;
        for (block, command) in blocks {
            writeln!(self.output, "  {}", block.label())?;
            for line in command.lines() {
                writeln!(self.output, "    {line}")?;
            }
        }
        self.output.flush()
    }

    pub fn summary(&mut self, documents: &[Document]) -> io::Result<()> {
        let passed_documents = documents.iter().filter(|doc| doc.succeeded()).count();
        let failed_documents = documents.iter().filter(|doc| doc.failed()).count();
        let cancelled_documents = documents
            .iter()
            .filter(|doc| doc.cancelled && !doc.failed() && !doc.not_run)
            .count();
        let not_run_documents = documents.iter().filter(|doc| doc.not_run).count();
        let passed_blocks: usize = documents.iter().map(|doc| doc.passed).sum();
        let failed_blocks: usize = documents.iter().map(|doc| doc.failed).sum();
        writeln!(
            self.output,
            "\n{BOLD}════════════════ Summary ════════════════{RESET}"
        )?;
        write!(
            self.output,
            "Documents: {passed_documents} passed, {failed_documents} failed"
        )?;
        if cancelled_documents > 0 {
            write!(self.output, ", {cancelled_documents} cancelled")?;
        }
        if not_run_documents > 0 {
            write!(self.output, ", {not_run_documents} not run")?;
        }
        writeln!(self.output)?;
        if passed_documents > 0 {
            writeln!(self.output, "\n{GREEN}PASS{RESET}")?;
            for document in documents.iter().filter(|doc| doc.succeeded()) {
                let line = format!(
                    "  ✓ {}  {}/{} blocks",
                    document.file, document.passed, document.total
                );
                writeln!(
                    self.output,
                    "{GREEN}{}{RESET}",
                    if self.tty {
                        fit(&line, terminal_width())
                    } else {
                        line
                    }
                )?;
            }
        }
        if failed_documents > 0 {
            writeln!(self.output, "\n{RED}FAIL{RESET}")?;
            for document in documents.iter().filter(|doc| doc.failed()) {
                let line = format!(
                    "  ✗ {}  {}/{} blocks",
                    document.file, document.passed, document.total
                );
                writeln!(
                    self.output,
                    "{RED}{}{RESET}",
                    if self.tty {
                        fit(&line, terminal_width())
                    } else {
                        line
                    }
                )?;
                for heading in &document.failed_headings {
                    let line = format!("    └─ {heading}");
                    writeln!(
                        self.output,
                        "{}",
                        if self.tty {
                            fit(&line, terminal_width())
                        } else {
                            line
                        }
                    )?;
                }
            }
        }
        if cancelled_documents > 0 {
            writeln!(self.output, "\n{YELLOW}CANCELLED{RESET}")?;
            for document in documents
                .iter()
                .filter(|doc| doc.cancelled && !doc.failed() && !doc.not_run)
            {
                let line = format!(
                    "  ■ {}  {}/{} blocks",
                    document.file, document.passed, document.total
                );
                writeln!(
                    self.output,
                    "{YELLOW}{}{RESET}",
                    if self.tty {
                        fit(&line, terminal_width())
                    } else {
                        line
                    }
                )?;
            }
        }
        if not_run_documents > 0 {
            writeln!(self.output, "\n{YELLOW}NOT RUN{RESET}")?;
            for document in documents.iter().filter(|doc| doc.not_run) {
                let line = format!("  - {}  0/{} blocks", document.file, document.total);
                writeln!(
                    self.output,
                    "{YELLOW}{}{RESET}",
                    if self.tty {
                        fit(&line, terminal_width())
                    } else {
                        line
                    }
                )?;
            }
        }
        writeln!(
            self.output,
            "\nDuration: {}",
            duration(self.started.elapsed())
        )?;
        writeln!(
            self.output,
            "Blocks: {passed_blocks} passed, {failed_blocks} failed (includes deferred)"
        )?;
        self.output.flush()
    }
}

pub struct Progress {
    stop: Option<mpsc::SyncSender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Progress {
    fn inactive() -> Self {
        Self {
            stop: None,
            thread: None,
        }
    }

    fn new(block: &Block) -> io::Result<Self> {
        Self::start(block, Duration::from_millis(80), true)
    }

    fn start(block: &Block, interval: Duration, render: bool) -> io::Result<Self> {
        let (stop, done) = mpsc::sync_channel(1);
        let label = block.label();
        let started = Instant::now();
        let thread = thread::Builder::new()
            .name("markmason-progress".into())
            .spawn(move || {
                let mut output = AutoStream::new(io::stdout(), ColorChoice::Auto);
                let mut terminal = io::stdout();
                let mut frame = 0;
                loop {
                    if render {
                        let width = crossterm::terminal::size()
                            .map(|(columns, _)| usize::from(columns))
                            .unwrap_or(80)
                            .saturating_sub(1);
                        let line = progress_line(
                            SPINNER[frame % SPINNER.len()],
                            &label,
                            &duration(started.elapsed()),
                            width,
                        );
                        let _ = terminal.write_all(b"\r\x1b[2K");
                        let _ = terminal.flush();
                        let _ = write!(output, "{CYAN}{line}{RESET}");
                        let _ = output.flush();
                    }
                    frame += 1;
                    match done.recv_timeout(interval) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
                if render {
                    let _ = terminal.write_all(b"\r\x1b[2K");
                    let _ = terminal.flush();
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    pub fn finish(mut self) -> io::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn heading(title: Option<&str>, command: &str) -> String {
    let value = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            command
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(str::trim)
        })
        .unwrap_or("(unnamed block)");
    fit(value, 72)
}

fn duration(value: Duration) -> String {
    if value < Duration::from_secs(1) {
        return "<1s".into();
    }
    let seconds = value.as_secs();
    if seconds < 60 {
        return format!("00:{seconds:02}");
    }
    format!("{:02}m {:02}s", seconds / 60, seconds % 60)
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(columns, _)| usize::from(columns).saturating_sub(1))
        .unwrap_or(79)
}

fn progress_line(spinner: char, label: &str, elapsed: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let prefix = format!("  {spinner} ");
    let suffix = format!("  {elapsed}");
    let suffix_width = measure_text_width(&suffix);
    if suffix_width >= width {
        return fit(&suffix, width);
    }
    let prefix_width = measure_text_width(&prefix);
    if prefix_width + suffix_width > width {
        return format!("{}{}", fit(&prefix, width - suffix_width), suffix);
    }
    let label_width = width - prefix_width - suffix_width;
    format!("{prefix}{}{suffix}", fit(label, label_width))
}

fn fit(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if measure_text_width(text) <= width {
        return text.to_owned();
    }
    if width == 1 {
        return "…".into();
    }
    let mut fitted = String::new();
    let mut cells = 0;
    for grapheme in text.graphemes(true) {
        let next = measure_text_width(grapheme);
        if cells + next + 1 > width {
            break;
        }
        fitted.push_str(grapheme);
        cells += next;
    }
    fitted.push('…');
    fitted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_shutdown_wakes_the_worker() -> io::Result<()> {
        let block = Block {
            position: 1,
            total: 1,
            heading: "brief command".into(),
            command: "app echo ready".into(),
            deferred: false,
        };
        let progress = Progress::start(&block, Duration::from_secs(5), false)?;
        thread::sleep(Duration::from_millis(10));
        let started = Instant::now();
        progress.finish()?;
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
        Ok(())
    }

    #[test]
    fn progress_line_clamps_ultra_narrow_terminals_and_keeps_timer_when_possible() {
        for (width, elapsed) in [(8, "<1s"), (10, "00:01")] {
            let line = progress_line('⠋', "a very long heading", elapsed, width);
            assert!(measure_text_width(&line) <= width, "{width}: {line:?}");
            assert!(line.contains(elapsed), "{width}: {line:?}");
        }
    }
}
