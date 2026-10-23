use crate::{cancellation, matching::pty_text, process, spec};
use anyhow::{Context, Result, bail};
use regex::bytes::Regex;
use std::{
    cell::RefCell,
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{LazyLock, mpsc},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

const BOOTSTRAP: &str = r#"set -uo pipefail
readonly __markmason_root=$1
readonly __markmason_requests=$2
__markmason_sleep=$(type -P sleep)
: "${__markmason_sleep:?shell: bash requires sleep on PATH}"
case "$__markmason_sleep" in
    /*) ;;
    *) __markmason_sleep="$PWD/$__markmason_sleep" ;;
esac
readonly __markmason_sleep
set --
command stty -echo || exit
builtin printf 'ready\n' >| "$__markmason_root/status"
__markmason_sequence=0
__markmason_status=0
__markmason_last_arg=
__markmason_flags=$-
__markmason_previous_status() {
    {
        case "$__markmason_flags" in *v*) set -v ;; esac
        case "$__markmason_flags" in *x*) set -x ;; esac
        return "$__markmason_status"
    } 2>/dev/null
}
while :; do
    __markmason_request="$__markmason_requests/command-$__markmason_sequence"
    while ! builtin test -f "$__markmason_request/ready"; do
        "$__markmason_sleep" 0.05 || exit
    done
    if builtin test -f "$__markmason_requests/cleanup" &&
        builtin test -z "$(builtin trap -p INT)"; then
        builtin trap 'return 130' INT
    fi
    __markmason_captured=
    if builtin test -f "$__markmason_request/pty"; then
        builtin . "$__markmason_request/script"
    else
        builtin . "$__markmason_request/script" < "$__markmason_request/input" >> "$__markmason_root/stdout" 2>> "$__markmason_root/stderr"
    fi
    {
        __markmason_source_status=$?
        if builtin test "$__markmason_captured" != yes; then
            __markmason_status=$__markmason_source_status __markmason_flags=$-
        fi
        set +xv
    } 2>/dev/null
    IFS= builtin read -r __markmason_boundary < "$__markmason_request/boundary"
    builtin printf '\r\n%s\r\n' "$__markmason_boundary"
    builtin printf '%s:%s\n' "$__markmason_sequence" "$__markmason_status" >| "$__markmason_root/status"
    __markmason_sequence=$((__markmason_sequence + 1))
done
"#;

pub fn commands(script: &str) -> Result<Vec<String>> {
    Ok(command_ranges(script)?
        .into_iter()
        .map(|range| script[range].to_owned())
        .collect())
}

pub fn command_ranges(script: &str) -> Result<Vec<std::ops::Range<usize>>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_bash::LANGUAGE.into())?;
    let tree = parser
        .parse(script, None)
        .context("Cannot parse Bash commands")?;
    if tree.root_node().has_error() {
        bail!("Invalid or incomplete Bash syntax in markmason block");
    }
    let mut cursor = tree.walk();
    let commands: Vec<_> = tree
        .root_node()
        .named_children(&mut cursor)
        .filter(|node| node.kind() != "comment")
        .map(|node| {
            let end = node
                .next_sibling()
                .filter(|next| next.kind() == "&")
                .map_or(node.end_byte(), |next| next.end_byte());
            node.start_byte()..end
        })
        .collect();
    if commands.is_empty() {
        bail!("A markmason block must contain nonempty commands");
    }
    Ok(commands)
}

pub fn path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    let text = text.replace('\\', "/");
    text
}

pub fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

pub struct Block<'a> {
    interaction: process::Interaction<'a>,
    raw: String,
    clean: String,
}

impl<'a> Block<'a> {
    pub fn new(steps: &'a [spec::Step]) -> Self {
        Self {
            interaction: process::Interaction::new(steps),
            raw: String::new(),
            clean: String::new(),
        }
    }
}

fn before_boundary<'a>(text: &'a str, boundary: &str, allow_eof: bool) -> Option<&'a str> {
    boundary_frame(text.as_bytes(), boundary, allow_eof).map(|(position, _)| {
        let text = &text[..position];
        text.strip_suffix("\r\r\n")
            .or_else(|| text.strip_suffix("\r\n"))
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(text)
    })
}

#[cfg(test)]
pub(super) fn boundary_end(bytes: &[u8], boundary: &str) -> Option<usize> {
    boundary_frame(bytes, boundary, false).map(|(_, end)| end)
}

#[cfg(test)]
pub(super) fn boundary_end_at_eof(bytes: &[u8], boundary: &str) -> Option<usize> {
    boundary_frame(bytes, boundary, true).map(|(_, end)| end)
}

fn boundary_frame(bytes: &[u8], boundary: &str, allow_eof: bool) -> Option<(usize, usize)> {
    static FRAME_END: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:\r|\x1b\[[?]?[0-9;]*[a-zA-Z])*?(?:\n|\x1b\[[0-9]+;1[Hf])").unwrap()
    });
    for (position, _) in bytes
        .windows(boundary.len())
        .enumerate()
        .filter(|(_, part)| *part == boundary.as_bytes())
    {
        let prefix = pty_text(&String::from_utf8_lossy(&bytes[..position]));
        if !prefix.is_empty() && !prefix.ends_with('\n') {
            continue;
        }
        let start = position + boundary.len();
        if allow_eof && start == bytes.len() {
            return Some((position, start));
        }
        if let Some(end) = FRAME_END.find(&bytes[start..]) {
            return Some((position, start + end.end()));
        }
    }
    None
}

fn protocol_file(path: &Path) -> Result<fs::File> {
    Ok(fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(path)?)
}

fn read_capture(file: &mut fs::File, offset: u64) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_protocol_text(file: &mut fs::File) -> Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(text)
}

pub(crate) fn recovery_needs_interrupt(status: Option<i32>, frame_end: Option<usize>) -> bool {
    status.is_none() && frame_end.is_none()
}

pub struct Session {
    terminal: Option<process::Terminal>,
    requests: TempDir,
    directory: TempDir,
    status: fs::File,
    sequence: usize,
    pending: Vec<u8>,
    stdout: RefCell<fs::File>,
    stderr: RefCell<fs::File>,
    stdout_position: u64,
    stderr_position: u64,
    cleanup_enabled: bool,
    ready: bool,
}

impl Session {
    pub fn new(
        cwd: &Path,
        env: &process::Environment,
        timeout: Duration,
        executor: &crate::sandbox::Executor,
        temporary_root: &Path,
    ) -> Result<Self> {
        let requests = tempfile::Builder::new()
            .prefix("markmason-shell-requests-")
            .tempdir_in(temporary_root)?;
        let directory = tempfile::Builder::new()
            .prefix("markmason-shell-")
            .tempdir_in(temporary_root)?;
        let bootstrap = requests.path().join("session.sh");
        fs::write(&bootstrap, BOOTSTRAP)?;
        // All guest-writable files are opened before launch. Later host writes
        // only use the separate directory that MXC exposes read-only.
        let status = protocol_file(&directory.path().join("status"))?;
        let stdout = protocol_file(&directory.path().join("stdout"))?;
        let stderr = protocol_file(&directory.path().join("stderr"))?;
        let mut env = env.clone();
        for name in ["BASH_ENV", "ENV", "SHELLOPTS", "BASHOPTS"] {
            process::set_env(&mut env, name, "");
        }
        let terminal = executor.with_shell_paths(requests.path(), directory.path())?.terminal(
            &[
                "bash".into(),
                "--noprofile".into(),
                "--norc".into(),
                path(&bootstrap),
                path(directory.path()),
                path(requests.path()),
            ],
            cwd,
            &env,
        )
        .context("shell: bash requires Bash on PATH; on native Windows install Git Bash, not a WSL launcher")?;
        let mut session = Self {
            terminal: Some(terminal),
            requests,
            directory,
            status,
            sequence: 0,
            pending: Vec::new(),
            stdout: RefCell::new(stdout),
            stderr: RefCell::new(stderr),
            stdout_position: 0,
            stderr_position: 0,
            cleanup_enabled: false,
            ready: true,
        };
        let deadline = Instant::now() + timeout.min(Duration::from_secs(10));
        let mut output = Vec::new();
        loop {
            cancellation::check()?;
            let terminal = session
                .terminal
                .as_mut()
                .context("Bash session is closed")?;
            while let Ok(chunk) = terminal.rx.try_recv() {
                output.extend(chunk.context("Failed reading Bash startup output")?);
            }
            if read_protocol_text(&mut session.status)? == "ready\n" {
                return Ok(session);
            }
            if terminal.try_wait()?.is_some() || Instant::now() >= deadline {
                bail!(
                    "Bash session failed to start. Install a local Bash (Git Bash on Windows).\n{}",
                    pty_text(&String::from_utf8_lossy(&output))
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn enable_cleanup(&mut self) -> Result<()> {
        fs::write(self.requests.path().join("cleanup"), "")?;
        self.cleanup_enabled = true;
        Ok(())
    }

    pub fn run(
        &mut self,
        command: &str,
        input: Option<Vec<u8>>,
        block: Option<&mut Block<'_>>,
        timeout: Duration,
        final_command: bool,
    ) -> Result<process::Output> {
        self.run_with_progress(
            command,
            input,
            block,
            timeout,
            final_command,
            &mut || Ok(()),
        )
    }

    pub fn run_with_progress(
        &mut self,
        command: &str,
        input: Option<Vec<u8>>,
        block: Option<&mut Block<'_>>,
        timeout: Duration,
        final_command: bool,
        progress: &mut dyn FnMut() -> Result<()>,
    ) -> Result<process::Output> {
        let result = self.execute(command, input, block, timeout, final_command, progress);
        if result.is_err()
            && !(self.cleanup_enabled && self.ready)
            && let Some(terminal) = self.terminal.take()
        {
            terminal
                .close()
                .context("Failed to close failed Bash session")?;
        }
        result
    }

    fn execute(
        &mut self,
        command: &str,
        input: Option<Vec<u8>>,
        mut block: Option<&mut Block<'_>>,
        timeout: Duration,
        final_command: bool,
        progress: &mut dyn FnMut() -> Result<()>,
    ) -> Result<process::Output> {
        let terminal = self.terminal.as_mut().context("Bash session is closed")?;
        let directory = self
            .requests
            .path()
            .join(format!("command-{}", self.sequence));
        fs::create_dir(&directory)?;
        fs::write(
            directory.join("script"),
            format!(
                "{{\nif __markmason_previous_status \"$__markmason_last_arg\"; then\n\
                 {command}\nelse\n{command}\nfi\n\
                 {{\n__markmason_status=$? __markmason_last_arg=${{_-}} __markmason_flags=$- __markmason_captured=yes\n\
                 set +xv\n}} 2>/dev/null\n}}\n"
            ),
        )?;
        fs::write(directory.join("input"), input.unwrap_or_default())?;
        let status_prefix = format!("{}:", self.sequence);
        if block.is_some() {
            fs::write(directory.join("pty"), "")?;
        }
        let boundary = format!(
            "MARKMASON_END_{}_{}",
            self.directory
                .path()
                .file_name()
                .context("Missing Bash session name")?
                .to_string_lossy(),
            self.sequence
        );
        fs::write(directory.join("boundary"), format!("{boundary}\n"))?;
        let mut bytes = std::mem::take(&mut self.pending);
        fs::write(directory.join("ready"), "")?;
        self.ready = false;
        self.sequence += 1;
        let start = Instant::now();
        let mut recovering = false;
        let mut interrupted = None;
        let mut recovery_deadline = None;
        let mut cleanup = None;
        let mut recovery_write = None;
        loop {
            progress()?;
            match terminal.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(chunk) => bytes.extend(chunk.context("Failed reading Bash terminal")?),
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let status = match read_protocol_text(&mut self.status) {
                Ok(text) if text.starts_with(&status_prefix) && text.ends_with('\n') => Some(
                    text[status_prefix.len()..]
                        .trim()
                        .parse::<i32>()
                        .context("Invalid Bash command status")?,
                ),
                Ok(_) => None,
                Err(error) => return Err(error).context("Failed reading Bash command status"),
            };
            let allow_eof = recovering
                && status.is_some()
                && recovery_deadline.is_some_and(|deadline| Instant::now() >= deadline);
            let raw = String::from_utf8_lossy(&bytes);
            let clean = pty_text(&raw);
            let finished = before_boundary(&clean, &boundary, allow_eof);
            let clean = finished.unwrap_or(&clean);
            let command_raw = before_boundary(&raw, &boundary, allow_eof).unwrap_or(&raw);
            let frame_end =
                boundary_frame(&bytes, &boundary, allow_eof).map(|(_, frame_end)| frame_end);
            if !recovering && let Some(block) = block.as_deref_mut() {
                block.interaction.advance(
                    &format!("{}{command_raw}", block.raw),
                    &format!("{}{clean}", block.clean),
                    &terminal.input,
                    status.is_none(),
                )?;
            }
            let output = |code| -> Result<(process::Output, u64, u64)> {
                let stdout = read_capture(&mut self.stdout.borrow_mut(), self.stdout_position)?;
                let stderr = read_capture(&mut self.stderr.borrow_mut(), self.stderr_position)?;
                let mut text = clean.to_owned();
                text.push_str(&String::from_utf8_lossy(&stdout));
                if code != 0 {
                    text.push_str(&String::from_utf8_lossy(&stderr));
                }
                Ok((
                    process::Output {
                        text,
                        code,
                        ..Default::default()
                    },
                    u64::try_from(stdout.len())?,
                    u64::try_from(stderr.len())?,
                ))
            };
            if !recovering {
                let cancellation = cancellation::check().err();
                let timed_out = start.elapsed() >= timeout;
                let interruption = cancellation.or_else(|| {
                    timed_out.then(|| {
                        anyhow::anyhow!(
                            "Bash command timed out after {}ms: {command}",
                            timeout.as_millis()
                        )
                    })
                });
                if let Some(error) = interruption {
                    let error = error.context(format!("Output so far:\n{}", output(1)?.0.text));
                    if !self.cleanup_enabled {
                        return Err(error);
                    }
                    recovering = true;
                    cleanup = Some(cancellation::Cleanup::new());
                    recovery_deadline = Some(Instant::now() + Duration::from_secs(2));
                    if recovery_needs_interrupt(status, frame_end) {
                        interrupted = Some(error);
                        recovery_write = Some(terminal.input.write(vec![3])?);
                    } else if cancellation::signal().is_some() {
                        interrupted = Some(error);
                    }
                }
            }
            if let Some(code) = status
                && (recovering
                    || block
                        .as_ref()
                        .is_none_or(|block| !block.interaction.sending()))
                && let Some(frame_end) = frame_end
            {
                let (output, stdout, stderr) = output(code)?;
                self.pending = bytes[frame_end..].to_vec();
                self.stdout_position += stdout;
                self.stderr_position += stderr;
                if recovering {
                    let deadline = recovery_deadline.context("Missing Bash recovery deadline")?;
                    if let Some(block) = block.as_deref_mut() {
                        block
                            .interaction
                            .settle_write(deadline.saturating_duration_since(Instant::now()))
                            .context("Could not finish interrupted PTY input write")?;
                    }
                    if let Some(written) = recovery_write {
                        written
                            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                            .context("Timed out writing Bash recovery interrupt")?
                            .context("Failed writing Bash recovery interrupt")?;
                    }
                    self.ready = true;
                    drop(cleanup);
                    if let Some(error) = interrupted {
                        return Err(error);
                    }
                } else {
                    self.ready = true;
                }
                if let Some(block) = block {
                    block.raw.push_str(command_raw);
                    block.clean.push_str(&output.text);
                    block
                        .interaction
                        .check_complete(final_command, code, &block.clean)?;
                }
                return Ok(output);
            }
            if terminal.try_wait()?.is_some() {
                bail!(
                    "Bash session exited before reporting command status.\nOutput:\n{}",
                    output(1)?.0.text
                );
            }
            if recovery_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Err(interrupted
                    .unwrap_or_else(|| {
                        anyhow::anyhow!(
                            "Bash command completed without a full terminal boundary: {command}"
                        )
                    })
                    .context("Bash session could not resume for deferred blocks"));
            }
        }
    }

    pub fn close(mut self) -> Result<()> {
        if let Some(terminal) = self.terminal.take() {
            terminal.close()?;
        }
        self.requests
            .close()
            .context("Failed to clean up Bash requests")?;
        self.directory
            .close()
            .context("Failed to clean up Bash session")
    }
}
