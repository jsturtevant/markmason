use crate::{cancellation, matching::pty_text, process, spec};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::Path,
    sync::mpsc,
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
builtin printf 'ready\n' > "$__markmason_root/ready"
__markmason_sequence=0
while :; do
    __markmason_request="$__markmason_root/command-$__markmason_sequence"
    while ! builtin test -f "$__markmason_request/ready"; do
        "$__markmason_sleep" 0.05 || exit
    done
    if builtin test -f "$__markmason_request/pty"; then
        builtin . "$__markmason_request/script"
    else
        builtin . "$__markmason_request/script" < "$__markmason_request/input" > "$__markmason_request/stdout" 2> "$__markmason_request/stderr"
    fi
    __markmason_status=$?
    if builtin test -f "$__markmason_request/pty"; then
        IFS= builtin read -r __markmason_boundary < "$__markmason_request/boundary"
        builtin printf '\r\n%s\r\n' "$__markmason_boundary"
    fi
    builtin printf '%s\n' "$__markmason_status" > "$__markmason_request/status"
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

pub struct Session {
    terminal: Option<process::Terminal>,
    directory: TempDir,
    sequence: usize,
}

impl Session {
    pub fn new(cwd: &Path, env: &process::Environment, timeout: Duration) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("markmason-shell-")
            .tempdir()?;
        let bootstrap = directory.path().join("session.sh");
        fs::write(&bootstrap, BOOTSTRAP)?;
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
        steps: Option<&[spec::Step]>,
        timeout: Duration,
        final_command: bool,
    ) -> Result<(process::Output, usize)> {
        let result = self.execute(command, input, steps, timeout, final_command);
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
        steps: Option<&[spec::Step]>,
        timeout: Duration,
        final_command: bool,
    ) -> Result<(process::Output, usize)> {
        let terminal = self.terminal.as_mut().context("Bash session is closed")?;
        let directory = self
            .directory
            .path()
            .join(format!("command-{}", self.sequence));
        fs::create_dir(&directory)?;
        fs::write(directory.join("script"), format!("{command}\n"))?;
        fs::write(directory.join("input"), input.unwrap_or_default())?;
        fs::write(directory.join("stdout"), "")?;
        fs::write(directory.join("stderr"), "")?;
        fs::write(directory.join("status"), "")?;
        if steps.is_some() {
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
        let mut bytes = Vec::new();
        while let Ok(chunk) = terminal.rx.try_recv() {
            bytes.extend(chunk.context("Failed reading Bash session output")?);
        }
        bytes.clear();
        fs::write(directory.join("ready"), "")?;
        self.sequence += 1;
        let start = Instant::now();
        let mut interaction = process::Interaction::new(steps.unwrap_or_default());
        loop {
            cancellation::check()?;
            match terminal.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(chunk) => bytes.extend(chunk.context("Failed reading Bash terminal")?),
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let raw = String::from_utf8_lossy(&bytes);
            let clean = pty_text(&raw);
            let boundary_position = clean.find(&boundary);
            let clean = boundary_position.map_or(clean.as_str(), |position| {
                clean[..position]
                    .strip_suffix("\r\n")
                    .or_else(|| clean[..position].strip_suffix('\n'))
                    .unwrap_or(&clean[..position])
            });
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
            interaction.advance(
                &raw,
                clean,
                &terminal.send_tx,
                &terminal.written_rx,
                status.is_none(),
            )?;
            let output = |code| -> Result<process::Output> {
                let text = if steps.is_some() {
                    clean.to_owned()
                } else {
                    let mut text =
                        String::from_utf8_lossy(&fs::read(directory.join("stdout"))?).into_owned();
                    if code != 0 {
                        text.push_str(&String::from_utf8_lossy(&fs::read(
                            directory.join("stderr"),
                        )?));
                    }
                    text
                };
                Ok(process::Output { text, code })
            };
            if let Some(code) = status
                && !interaction.sending()
                && (steps.is_none() || boundary_position.is_some())
            {
                if steps.is_some() {
                    interaction.check_complete(final_command, code, clean)?;
                    return Ok((
                        process::Output {
                            text: clean.to_owned(),
                            code,
                        },
                        interaction.completed(),
                    ));
                }
                return Ok((output(code)?, 0));
            }
            if terminal.child.try_wait()?.is_some() {
                bail!(
                    "Bash session exited before reporting command status.\nOutput:\n{}",
                    output(1)?.text
                );
            }
            if start.elapsed() >= timeout {
                bail!(
                    "Bash command timed out after {}ms: {command}\nOutput so far:\n{}",
                    timeout.as_millis(),
                    output(1)?.text
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
