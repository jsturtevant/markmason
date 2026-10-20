use anstream::{AutoStream, ColorChoice};
use dialoguer::console::measure_text_width;
use std::{
    io::{self, IsTerminal, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
    pub duration: Duration,
}

impl Document {
    pub fn failed(&self) -> bool {
        self.failed > 0 || self.errors > 0
    }

    pub fn succeeded(&self) -> bool {
        !self.failed() && !self.cancelled
    }
}

pub struct Presenter {
    tty: bool,
    started: Instant,
    output: AutoStream<std::io::Stdout>,
}

impl Presenter {
    pub fn new() -> Self {
        let tty = io::stdout().is_terminal()
            && std::env::var_os("TERM").is_none_or(|term| term != "dumb");
        Self {
            tty,
            started: Instant::now(),
            output: AutoStream::new(io::stdout(), ColorChoice::Auto),
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

    pub fn block_pass(&mut self, progress: Progress, block: &Block) -> io::Result<()> {
        progress.finish()?;
        if !self.tty || block.deferred {
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
    ) -> io::Result<()> {
        progress.finish()?;
        writeln!(self.output, "  {RED}✗ FAIL{RESET}  {}", block.label())?;
        writeln!(self.output, "    Document: {file}")?;
        writeln!(self.output, "    Command: {}", block.command)?;
        for line in format!("{error:#}").lines() {
            writeln!(self.output, "    {line}")?;
        }
        self.output.flush()
    }

    pub fn block_cancel(&mut self, progress: Progress, block: &Block) -> io::Result<()> {
        progress.finish()?;
        writeln!(
            self.output,
            "  {YELLOW}■ CANCELLED{RESET}  {}",
            block.label()
        )?;
        self.output.flush()
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
            .filter(|doc| doc.cancelled && !doc.failed())
            .count();
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
                .filter(|doc| doc.cancelled && !doc.failed())
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
        writeln!(
            self.output,
            "\nBlocks: {passed_blocks} passed, {failed_blocks} failed (includes deferred)"
        )?;
        writeln!(
            self.output,
            "Duration: {}",
            duration(self.started.elapsed())
        )?;
        self.output.flush()
    }
}

pub struct Progress {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Progress {
    fn inactive() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(true)),
            thread: None,
        }
    }

    fn new(block: &Block) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::clone(&stop);
        let label = block.label();
        let started = Instant::now();
        let thread = thread::Builder::new()
            .name("markmason-progress".into())
            .spawn(move || {
                let mut output = AutoStream::new(io::stdout(), ColorChoice::Auto);
                let mut terminal = io::stdout();
                let mut frame = 0;
                while !done.load(Ordering::Relaxed) {
                    let width = crossterm::terminal::size()
                        .map(|(columns, _)| usize::from(columns))
                        .unwrap_or(80);
                    let suffix = duration(started.elapsed());
                    let line = fit(
                        &format!("  {} {label}  {suffix}", SPINNER[frame % SPINNER.len()]),
                        width.saturating_sub(1),
                    );
                    let _ = terminal.write_all(b"\r\x1b[2K");
                    let _ = terminal.flush();
                    let _ = write!(output, "{CYAN}{line}{RESET}");
                    let _ = output.flush();
                    frame += 1;
                    thread::sleep(Duration::from_millis(80));
                }
                let _ = terminal.write_all(b"\r\x1b[2K");
                let _ = terminal.flush();
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
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

pub fn first_command(command: &str) -> String {
    command
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("(empty command)")
        .to_owned()
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
