use crate::{cancellation, matching::pty_text, process, spec};
use anyhow::{Context, Result, bail};
use regex::bytes::Regex;
use std::{
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
readonly __markmason_sleep=$(type -P sleep)
: "${__markmason_sleep:?shell: bash requires sleep on PATH}"
set --
command stty -echo || exit
builtin printf 'ready\n' >| "$__markmason_root/ready"
__markmason_sequence=0
__markmason_status=0
__markmason_last_arg=
__markmason_previous_status() { return "$__markmason_status"; }
while :; do
    __markmason_request="$__markmason_root/command-$__markmason_sequence"
    while ! builtin test -f "$__markmason_request/ready"; do
        "$__markmason_sleep" 0.05 || exit
    done
    if builtin test -f "$__markmason_request/pty"; then
        if __markmason_previous_status "$__markmason_last_arg"; then
            builtin . "$__markmason_request/script"
        else
            builtin . "$__markmason_request/script"
        fi
    else
        if __markmason_previous_status "$__markmason_last_arg"; then
            builtin . "$__markmason_request/script" < "$__markmason_request/input" >> "$__markmason_root/stdout" 2>> "$__markmason_root/stderr"
        else
            builtin . "$__markmason_request/script" < "$__markmason_request/input" >> "$__markmason_root/stdout" 2>> "$__markmason_root/stderr"
        fi
    fi
    __markmason_status=$?
    IFS= builtin read -r __markmason_boundary < "$__markmason_request/boundary"
    builtin printf '\r\n%s\r\n' "$__markmason_boundary"
    builtin printf '%s\n' "$__markmason_status" >| "$__markmason_request/status"
    __markmason_sequence=$((__markmason_sequence + 1))
done
"#;

pub fn commands(script: &str) -> Result<Vec<String>> {
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
            script[node.start_byte()..end].to_owned()
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

fn before_boundary<'a>(text: &'a str, boundary: &str) -> Option<&'a str> {
    text.find(boundary).map(|position| {
        let text = &text[..position];
        text.strip_suffix("\r\r\n")
            .or_else(|| text.strip_suffix("\r\n"))
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(text)
    })
}

pub(super) fn boundary_end(bytes: &[u8], boundary: &str) -> Option<usize> {
    static LINE_START: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\x1b\[[0-9]+;1[Hf]").unwrap());
    let start = bytes
        .windows(boundary.len())
        .position(|part| part == boundary.as_bytes())?
        + boundary.len();
    let suffix = &bytes[start..];
    let newline = suffix
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|index| index + 1);
    let cursor = LINE_START.find(suffix).map(|matched| matched.end());
    newline
        .into_iter()
        .chain(cursor)
        .min()
        .map(|end| start + end)
}

fn read_capture(path: &Path, offset: u64) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub struct Session {
    terminal: Option<process::Terminal>,
    directory: TempDir,
    sequence: usize,
    pending: Vec<u8>,
    stdout_position: u64,
    stderr_position: u64,
}

impl Session {
    pub fn new(cwd: &Path, env: &process::Environment, timeout: Duration) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("markmason-shell-")
            .tempdir()?;
        let bootstrap = directory.path().join("session.sh");
        fs::write(&bootstrap, BOOTSTRAP)?;
        fs::write(directory.path().join("stdout"), "")?;
        fs::write(directory.path().join("stderr"), "")?;
        let mut env = env.clone();
        for name in ["BASH_ENV", "ENV", "SHELLOPTS", "BASHOPTS"] {
            process::set_env(&mut env, name, "");
        }
        let terminal = process::Terminal::new(
            &[
                "bash".into(),
                "--noprofile".into(),
                "--norc".into(),
                path(&bootstrap),
                path(directory.path()),
            ],
            cwd,
            &env,
        )
        .context("shell: bash requires Bash on PATH; on native Windows install Git Bash, not a WSL launcher")?;
        let mut session = Self {
            terminal: Some(terminal),
            directory,
            sequence: 0,
            pending: Vec::new(),
            stdout_position: 0,
            stderr_position: 0,
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
            if session.directory.path().join("ready").is_file() {
                return Ok(session);
            }
            if terminal.child.try_wait()?.is_some() || Instant::now() >= deadline {
                bail!(
                    "Bash session failed to start. Install a local Bash (Git Bash on Windows).\n{}",
                    pty_text(&String::from_utf8_lossy(&output))
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn run(
        &mut self,
        command: &str,
        input: Option<Vec<u8>>,
        block: Option<&mut Block<'_>>,
        timeout: Duration,
        final_command: bool,
    ) -> Result<process::Output> {
        let result = self.execute(command, input, block, timeout, final_command);
        if result.is_err()
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
    ) -> Result<process::Output> {
        let terminal = self.terminal.as_mut().context("Bash session is closed")?;
        let directory = self
            .directory
            .path()
            .join(format!("command-{}", self.sequence));
        fs::create_dir(&directory)?;
        fs::write(
            directory.join("script"),
            format!(
                "{command}\n__markmason_status=$? __markmason_last_arg=${{_-}}\n__markmason_previous_status \"$__markmason_last_arg\"\n"
            ),
        )?;
        fs::write(directory.join("input"), input.unwrap_or_default())?;
        fs::write(directory.join("status"), "")?;
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
        self.sequence += 1;
        let start = Instant::now();
        loop {
            cancellation::check()?;
            match terminal.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(chunk) => bytes.extend(chunk.context("Failed reading Bash terminal")?),
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let raw = String::from_utf8_lossy(&bytes);
            let clean = pty_text(&raw);
            let finished = before_boundary(&clean, &boundary);
            let clean = finished.unwrap_or(&clean);
            let command_raw = before_boundary(&raw, &boundary).unwrap_or(&raw);
            let frame_end = boundary_end(&bytes, &boundary);
            let status = match fs::read_to_string(directory.join("status")) {
                Ok(text) if text.ends_with('\n') => Some(
                    text.trim()
                        .parse::<i32>()
                        .context("Invalid Bash command status")?,
                ),
                Ok(_) => None,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error).context("Failed reading Bash command status"),
            };
            if let Some(block) = block.as_deref_mut() {
                block.interaction.advance(
                    &format!("{}{command_raw}", block.raw),
                    &format!("{}{clean}", block.clean),
                    &terminal.send_tx,
                    &terminal.written_rx,
                    status.is_none(),
                )?;
            }
            let output = |code| -> Result<(process::Output, u64, u64)> {
                let stdout =
                    read_capture(&self.directory.path().join("stdout"), self.stdout_position)?;
                let stderr =
                    read_capture(&self.directory.path().join("stderr"), self.stderr_position)?;
                let mut text = clean.to_owned();
                text.push_str(&String::from_utf8_lossy(&stdout));
                if code != 0 {
                    text.push_str(&String::from_utf8_lossy(&stderr));
                }
                Ok((
                    process::Output { text, code },
                    u64::try_from(stdout.len())?,
                    u64::try_from(stderr.len())?,
                ))
            };
            if let Some(code) = status
                && block
                    .as_ref()
                    .is_none_or(|block| !block.interaction.sending())
                && let Some(frame_end) = frame_end
            {
                let (output, stdout, stderr) = output(code)?;
                if let Some(block) = block {
                    block.raw.push_str(command_raw);
                    block.clean.push_str(&output.text);
                    block
                        .interaction
                        .check_complete(final_command, code, &block.clean)?;
                }
                self.pending = bytes[frame_end..].to_vec();
                self.stdout_position += stdout;
                self.stderr_position += stderr;
                return Ok(output);
            }
            if terminal.child.try_wait()?.is_some() {
                bail!(
                    "Bash session exited before reporting command status.\nOutput:\n{}",
                    output(1)?.0.text
                );
            }
            if start.elapsed() >= timeout {
                bail!(
                    "Bash command timed out after {}ms: {command}\nOutput so far:\n{}",
                    timeout.as_millis(),
                    output(1)?.0.text
                );
            }
        }
    }

    pub fn close(mut self) -> Result<()> {
        if let Some(terminal) = self.terminal.take() {
            terminal.close()?;
        }
        self.directory
            .close()
            .context("Failed to clean up Bash session")
    }
}
