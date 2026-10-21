use crate::{cancellation, matching::pty_text, spec};
use anyhow::{Context, Result, bail};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

pub type Environment = BTreeMap<OsString, OsString>;

#[derive(Default)]
pub struct Scope {
    #[cfg(windows)]
    jobs: std::cell::RefCell<Vec<crate::windows::ProcessTree>>,
}

impl Scope {
    pub fn close(self) -> Result<()> {
        #[cfg(windows)]
        for job in self.jobs.into_inner() {
            job.close()?;
        }
        Ok(())
    }
}

pub fn set_env(env: &mut Environment, key: &str, value: impl Into<OsString>) {
    #[cfg(windows)]
    env.retain(|name, _| !crate::windows::same_env_name(name, key));
    env.insert(key.into(), value.into());
}

#[derive(Debug)]
pub struct Output {
    pub text: String,
    pub code: i32,
}

#[cfg(windows)]
fn executable(program: &str, cwd: &Path, env: &Environment) -> Result<std::path::PathBuf> {
    let get_env = |key: &str| {
        env.iter()
            .find(|(name, _)| crate::windows::same_env_name(name, key))
            .map(|(_, value)| value)
    };
    let cwd = dunce::canonicalize(cwd)?;
    let path = std::path::PathBuf::from(program);
    let dirs = if path.components().count() > 1 || path.is_absolute() {
        vec![cwd.to_owned()]
    } else {
        std::iter::once(cwd.to_owned())
            .chain(
                std::env::split_paths(get_env("PATH").context("PATH is not set")?)
                    .map(|dir| cwd.join(dir)),
            )
            .collect()
    };
    let extensions = get_env("PATHEXT")
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_owned());
    for dir in dirs {
        let candidate = dir.join(&path);
        if path.extension().is_some() && candidate.is_file() {
            return Ok(dunce::canonicalize(candidate)?);
        }
        for extension in extensions.split(';').filter(|ext| !ext.is_empty()) {
            let mut candidate = candidate.as_os_str().to_owned();
            candidate.push(extension);
            let candidate = std::path::PathBuf::from(candidate);
            if candidate.is_file() {
                return Ok(dunce::canonicalize(candidate)?);
            }
        }
    }
    bail!("Executable not found on PATH: {program}");
}

#[cfg(windows)]
pub fn internal_exec() -> ! {
    let mut args = std::env::args_os().skip(2);
    let event = args.next();
    let status_path = args.next();
    let result = (|| {
        let event = event.context("Missing internal startup event")?;
        let status_path = status_path
            .as_ref()
            .context("Missing internal startup status")?;
        crate::windows::wait_for_start(&event)?;
        crate::windows::protect_stdio()?;
        // Keep the relay alive while its child handles console interrupts.
        cancellation::install()?;
        let program = args.next().context("Missing internal executable")?;
        let mut child = Command::new(program).args(args).spawn()?;
        std::fs::write(status_path, "started").context("Failed to report command startup")?;
        Ok::<_, anyhow::Error>(child.wait()?)
    })();
    match result {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            if let Some(path) = status_path
                && let Err(report_error) = std::fs::write(path, format!("{error:#}"))
            {
                eprintln!("Failed to report command startup error: {report_error}");
            }
            eprintln!("Failed to execute command: {error:#}");
            std::process::exit(1);
        }
    }
}

fn read_pipe(mut reader: impl Read + Send + 'static) -> Receiver<std::io::Result<Vec<u8>>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = reader.read_to_end(&mut bytes).map(|_| bytes);
        let _ = tx.send(result);
    });
    rx
}

#[cfg(unix)]
fn kill_group(pid: u32) -> Result<()> {
    let send = |signal| -> Result<()> {
        let result = unsafe { libc::kill(-(pid as i32), signal) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error.into());
            }
        }
        Ok(())
    };
    if let Some(signal) = cancellation::signal() {
        send(signal)?;
        thread::sleep(Duration::from_millis(100));
    }
    // Descendants can outlive the command and keep its output pipes open.
    send(libc::SIGKILL)
}

#[cfg(target_os = "linux")]
fn unix_pids() -> Result<Vec<i32>> {
    let mut pids = Vec::new();
    for entry in std::fs::read_dir("/proc").context("Failed listing process IDs")? {
        let entry = entry?;
        if let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() {
            pids.push(pid);
        }
    }
    Ok(pids)
}

#[cfg(target_os = "macos")]
fn unix_pids() -> Result<Vec<i32>> {
    let mut pids = vec![0i32; 1024];
    loop {
        let size = i32::try_from(std::mem::size_of_val(pids.as_slice()))?;
        // libproc's PROC_ALL_PIDS selector is not exported by libc.
        let count = unsafe { libc::proc_listpids(1, 0, pids.as_mut_ptr().cast(), size) };
        if count <= 0 {
            return Err(std::io::Error::last_os_error()).context("Failed listing process IDs");
        }
        if count < size {
            pids.truncate(usize::try_from(count)? / std::mem::size_of::<i32>());
            pids.retain(|pid| *pid > 0);
            return Ok(pids);
        }
        pids.resize(
            pids.len().checked_mul(2).context("Too many process IDs")?,
            0,
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn kill_terminal_session(session: u32) -> Result<()> {
    let mut groups = BTreeMap::<u32, Vec<i32>>::new();
    for pid in unix_pids()? {
        let owner = unsafe { libc::getsid(pid) };
        if owner == -1 {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(libc::ESRCH | libc::EPERM)) {
                continue;
            }
            return Err(error).context("Failed reading process session");
        }
        if owner == session as i32 {
            let group = unsafe { libc::getpgid(pid) };
            if group == -1 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ESRCH) {
                    continue;
                }
                return Err(error).context("Failed reading terminal process group");
            }
            groups.entry(u32::try_from(group)?).or_default().push(pid);
        }
    }
    for (group, members) in groups {
        if let Err(error) = kill_group(group) {
            #[cfg(target_os = "macos")]
            if error
                .downcast_ref::<std::io::Error>()
                .and_then(|error| error.raw_os_error())
                == Some(libc::EPERM)
            {
                // Darwin rejects killpg for zombie-only groups; individual kill succeeds.
                for pid in members {
                    if unsafe { libc::kill(pid, libc::SIGKILL) } != 0 {
                        let error = std::io::Error::last_os_error();
                        if error.raw_os_error() != Some(libc::ESRCH) {
                            return Err(error).with_context(|| {
                                format!("Failed terminating owned process {pid}")
                            });
                        }
                    }
                }
                continue;
            }
            #[cfg(not(target_os = "macos"))]
            let _ = members;
            return Err(error).with_context(|| {
                format!("Failed terminating group {group} in terminal session {session}")
            });
        }
    }
    Ok(())
}

pub fn run(
    args: &[String],
    cwd: &Path,
    env: &Environment,
    input: Option<Vec<u8>>,
    timeout: Duration,
    scope: &Scope,
) -> Result<Output> {
    run_with_progress(args, cwd, env, input, timeout, scope, &mut || Ok(()))
}

pub fn run_with_progress(
    args: &[String],
    cwd: &Path,
    env: &Environment,
    input: Option<Vec<u8>>,
    timeout: Duration,
    scope: &Scope,
    progress: &mut dyn FnMut() -> Result<()>,
) -> Result<Output> {
    #[cfg(not(windows))]
    let _ = scope;
    cancellation::check()?;
    let program = args.first().context("Empty command in pipeline stage")?;
    #[cfg(windows)]
    let program =
        executable(program, cwd, env).with_context(|| format!("Failed to start {program}"))?;
    #[cfg(windows)]
    let tree = crate::windows::ProcessTree::new()?;
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new(std::env::current_exe()?);
        command.args([
            OsString::from("--internal-exec"),
            tree.event_name.clone(),
            tree.status_path.clone().into_os_string(),
            program.clone().into_os_string(),
        ]);
        command
    };
    #[cfg(not(windows))]
    let mut command = Command::new(program);
    command
        .args(&args[1..])
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("Failed to start {program:?}"))?;
    #[cfg(windows)]
    if let Err(error) = tree.start(child.id()) {
        child
            .kill()
            .context("Failed to terminate command after startup failure")?;
        child
            .wait()
            .context("Failed to reap command after startup failure")?;
        return Err(error);
    }
    let terminate = |child: &mut Child| -> Result<()> {
        #[cfg(unix)]
        kill_group(child.id())?;
        #[cfg(windows)]
        tree.terminate()?;
        child.wait()?;
        Ok(())
    };
    let stdout = read_pipe(child.stdout.take().context("Missing stdout pipe")?);
    let stderr = read_pipe(child.stderr.take().context("Missing stderr pipe")?);
    let mut stdin = child.stdin.take().context("Missing stdin pipe")?;
    let (input_tx, input_rx) = mpsc::channel();
    thread::spawn(move || {
        let result = if let Some(input) = input {
            stdin.write_all(&input)
        } else {
            Ok(())
        };
        drop(stdin);
        let _ = input_tx.send(result);
    });
    let mut terminated = false;
    let result = (|| {
        let start = Instant::now();
        let status = loop {
            cancellation::check()?;
            progress()?;
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() >= timeout {
                terminate(&mut child)?;
                terminated = true;
                let out = stdout
                    .recv_timeout(Duration::from_secs(1))
                    .ok()
                    .transpose()?;
                let err = stderr
                    .recv_timeout(Duration::from_secs(1))
                    .ok()
                    .transpose()?;
                bail!(
                    "Command timed out after {}ms: {}\nOutput so far:\n{}{}",
                    timeout.as_millis(),
                    args.join(" "),
                    String::from_utf8_lossy(&out.unwrap_or_default()),
                    String::from_utf8_lossy(&err.unwrap_or_default())
                );
            }
            thread::sleep(Duration::from_millis(10));
        };
        #[cfg(windows)]
        tree.check_started()?;
        let mut collect = |rx: Receiver<std::io::Result<Vec<u8>>>| -> Result<Vec<u8>> {
            loop {
                cancellation::check()?;
                progress()?;
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(result) => return Ok(result?),
                    Err(mpsc::RecvTimeoutError::Timeout) if start.elapsed() < timeout => {}
                    Err(error) => {
                        bail!("Timed out draining command output: {error}");
                    }
                }
            }
        };
        let stdout = collect(stdout)?;
        let stderr = collect(stderr)?;
        let input_result = loop {
            cancellation::check()?;
            progress()?;
            match input_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => break result,
                Err(mpsc::RecvTimeoutError::Timeout) if start.elapsed() < timeout => {}
                Err(error) => bail!("Failed waiting for stdin writer: {error}"),
            }
        };
        if let Err(error) = input_result
            && error.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(error).context("Failed writing stdin");
        }
        let code = status.code().unwrap_or(1);
        let mut text = String::from_utf8_lossy(&stdout).into_owned();
        if code != 0 {
            text.push_str(&String::from_utf8_lossy(&stderr));
        }
        Ok(Output { text, code })
    })();
    if result.is_err() && !terminated {
        terminate(&mut child).context("Failed to clean up command process group")?;
    }
    #[cfg(windows)]
    if result.is_ok() {
        scope.jobs.borrow_mut().push(tree);
    }
    result
}

pub struct Terminal {
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    pub rx: Receiver<std::io::Result<Vec<u8>>>,
    pub input: TerminalInput,
    #[cfg(windows)]
    tree: Option<crate::windows::ProcessTree>,
    finished: bool,
}

struct WriteRequest {
    input: Vec<u8>,
    completed: mpsc::SyncSender<std::io::Result<()>>,
}

#[derive(Clone)]
pub struct TerminalInput {
    send: mpsc::Sender<WriteRequest>,
}

impl TerminalInput {
    pub fn write(&self, input: Vec<u8>) -> Result<Receiver<std::io::Result<()>>> {
        let (completed, result) = mpsc::sync_channel(1);
        self.send
            .send(WriteRequest { input, completed })
            .context("PTY input writer stopped")?;
        Ok(result)
    }
}

impl Terminal {
    pub fn new(args: &[String], cwd: &Path, env: &Environment) -> Result<Self> {
        cancellation::check()?;
        let pair = native_pty_system().openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        #[cfg(not(windows))]
        let argv = args.iter().map(OsString::from).collect();
        #[cfg(windows)]
        let tree = crate::windows::ProcessTree::new()?;
        #[cfg(windows)]
        let argv = {
            let program = executable(args.first().context("Empty PTY command")?, cwd, env)?;
            let mut argv = vec![
                std::env::current_exe()?.into_os_string(),
                "--internal-exec".into(),
                tree.event_name.clone(),
                tree.status_path.clone().into_os_string(),
            ];
            argv.push(program.into_os_string());
            argv.extend(args.iter().skip(1).map(OsString::from));
            argv
        };
        let mut command = CommandBuilder::from_argv(argv);
        command.cwd(cwd);
        command.env_clear();
        for (key, value) in env {
            command.env(key, value);
        }
        command.env("TERM", "xterm-color");
        let mut reader = pair.master.try_clone_reader()?;
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let (tx, rx) = mpsc::channel();
        #[cfg(windows)]
        let response_writer = Arc::clone(&writer);
        thread::spawn(move || {
            let mut buffer = [0; 8192];
            #[cfg(windows)]
            let mut queries = Vec::new();
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        #[cfg(windows)]
                        {
                            // ConPTY's INHERIT_CURSOR handshake can precede spawn completion.
                            queries.extend_from_slice(&buffer[..n]);
                            let result = (|| -> Result<()> {
                                for _ in queries.windows(4).filter(|bytes| *bytes == b"\x1b[6n") {
                                    let mut writer = response_writer.lock().unwrap();
                                    writer.write_all(b"\x1b[1;1R")?;
                                    writer.flush()?;
                                }
                                Ok(())
                            })();
                            if let Err(error) = result {
                                let _ = tx.send(Err(std::io::Error::other(error)));
                                break;
                            }
                            queries.drain(..queries.len().saturating_sub(3));
                        }
                        if tx.send(Ok(buffer[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    // Linux PTYs return EIO when the slave closes.
                    Err(error) if cfg!(unix) && error.raw_os_error() == Some(5) => break,
                    Err(error) => {
                        let _ = tx.send(Err(error));
                        break;
                    }
                }
            }
        });
        let child = pair.slave.spawn_command(command)?;
        #[cfg(windows)]
        let mut child = child;
        #[cfg(windows)]
        if let Err(error) = tree.start(child.process_id().context("Missing PTY process ID")?) {
            child
                .kill()
                .context("Failed to terminate PTY after startup failure")?;
            child
                .wait()
                .context("Failed to reap PTY after startup failure")?;
            return Err(error);
        }
        drop(pair.slave);
        let master = Some(pair.master);
        let (send_tx, send_rx) = mpsc::channel::<WriteRequest>();
        thread::spawn(move || {
            for request in send_rx {
                let mut writer = writer.lock().unwrap();
                let result = writer
                    .write_all(&request.input)
                    .and_then(|()| writer.flush());
                let failed = result.is_err();
                if request.completed.send(result).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            master,
            rx,
            input: TerminalInput { send: send_tx },
            #[cfg(windows)]
            tree: Some(tree),
            finished: false,
        })
    }

    fn terminate(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        kill_terminal_session(self.child.process_id().context("Missing PTY process ID")?)?;
        #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
        kill_group(self.child.process_id().context("Missing PTY process ID")?)?;
        #[cfg(windows)]
        if let Some(tree) = &self.tree {
            tree.terminate()?;
        }
        if self.child.try_wait()?.is_none()
            && let Err(error) = self.child.kill()
            && self.child.try_wait()?.is_none()
        {
            return Err(error).context("Failed to terminate PTY command");
        }
        self.child.wait().context("Failed to reap PTY command")?;
        drop(self.master.take());
        self.finished = true;
        Ok(())
    }

    pub fn close(mut self) -> Result<()> {
        self.terminate()?;
        #[cfg(windows)]
        if let Some(tree) = self.tree.take() {
            tree.close()?;
        }
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Err(error) = self.terminate() {
            eprintln!("Failed to clean up terminal session: {error:#}");
        }
    }
}

pub struct Interaction<'a> {
    steps: &'a [spec::Step],
    index: usize,
    raw_cursor: usize,
    clean_cursor: usize,
    pending: Option<Instant>,
    sending: Option<Receiver<std::io::Result<()>>>,
}

impl<'a> Interaction<'a> {
    pub fn new(steps: &'a [spec::Step]) -> Self {
        Self {
            steps,
            index: 0,
            raw_cursor: 0,
            clean_cursor: 0,
            pending: None,
            sending: None,
        }
    }

    pub fn advance(
        &mut self,
        raw: &str,
        clean: &str,
        input: &TerminalInput,
        running: bool,
    ) -> Result<()> {
        if let Some(written) = &self.sending {
            match written.try_recv() {
                Ok(result) => {
                    result.context("Failed writing PTY input")?;
                    self.index += 1;
                    self.sending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(error) => bail!("PTY input writer stopped: {error}"),
            }
        } else if let Some(send_at) = self.pending {
            if Instant::now() >= send_at && running {
                self.sending = Some(
                    input.write(spec::input_keys(&self.steps[self.index].input).into_bytes())?,
                );
                self.pending = None;
            }
        } else if let Some(step) = self.steps.get(self.index) {
            let (haystack, cursor) = if step.raw {
                (raw, &mut self.raw_cursor)
            } else {
                (clean, &mut self.clean_cursor)
            };
            if let Some(pos) = haystack
                .get(*cursor..)
                .and_then(|text| text.find(&step.text))
            {
                *cursor += pos + step.text.len();
                self.pending = Some(Instant::now() + Duration::from_millis(200));
            }
        }
        Ok(())
    }

    pub fn completed(&self) -> usize {
        self.index
    }

    pub fn sending(&self) -> bool {
        self.sending.is_some()
    }

    pub fn settle_write(&mut self, timeout: Duration) -> Result<()> {
        let Some(written) = self.sending.take() else {
            return Ok(());
        };
        written
            .recv_timeout(timeout)
            .context("Timed out waiting for PTY input write")?
            .context("Failed writing PTY input")?;
        self.index += 1;
        Ok(())
    }

    pub fn check_complete(&self, final_command: bool, code: i32, output: &str) -> Result<()> {
        if (final_command || code != 0) && self.index < self.steps.len() {
            bail!(
                "PTY exited before completing step {}: match {:?}\nOutput:\n{output}",
                self.index,
                self.steps[self.index].text
            );
        }
        Ok(())
    }
}

pub struct PtyExecution<'a> {
    pub scope: &'a Scope,
    pub final_command: bool,
    pub progress: &'a mut dyn FnMut() -> Result<()>,
}

pub fn pty(
    args: &[String],
    cwd: &Path,
    env: &Environment,
    steps: &[spec::Step],
    timeout: Duration,
    execution: PtyExecution<'_>,
) -> Result<(Output, usize)> {
    let PtyExecution {
        scope,
        final_command,
        progress,
    } = execution;
    #[cfg(not(windows))]
    let _ = scope;
    let mut terminal = Terminal::new(args, cwd, env)?;
    let Terminal {
        child,
        master,
        rx,
        input,
        #[cfg(windows)]
        tree,
        ..
    } = &mut terminal;
    let result = (|| {
        let mut bytes = Vec::new();
        let mut interaction = Interaction::new(steps);
        let start = Instant::now();
        let mut exited = None;
        let mut eof = false;
        loop {
            cancellation::check()?;
            progress()?;
            match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(chunk) => bytes.extend(chunk.context("Failed reading PTY output")?),
                Err(mpsc::RecvTimeoutError::Disconnected) => eof = true,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let raw = String::from_utf8_lossy(&bytes);
            let clean = pty_text(&raw);
            if start.elapsed() >= timeout {
                let step_index = interaction.completed();
                let waiting = steps.get(step_index).map_or_else(
                    || "Waiting for process to exit (all steps matched)".to_owned(),
                    |step| format!("Waiting for step {step_index}: match {:?}", step.text),
                );
                bail!(
                    "PTY command timed out after {}ms.\n{waiting}\nOutput so far:\n{clean}",
                    timeout.as_millis()
                );
            }
            interaction.advance(&raw, &clean, input, exited.is_none())?;
            if exited.is_none() {
                exited = child.try_wait()?;
                if exited.is_some() {
                    #[cfg(windows)]
                    tree.as_ref().context("Missing PTY job")?.check_started()?;
                    // Closing ConPTY releases its output pipe so the reader can drain to EOF.
                    drop(master.take());
                }
            }
            if let Some(status) = &exited
                && eof
                && !interaction.sending()
            {
                let code = status.exit_code() as i32;
                interaction.check_complete(final_command, code, &clean)?;
                return Ok((Output { text: clean, code }, interaction.completed()));
            }
        }
    })();
    #[cfg(windows)]
    if result.is_ok() {
        scope
            .jobs
            .borrow_mut()
            .push(terminal.tree.take().context("Missing PTY job")?);
    }
    if result.is_err() {
        terminal.close()?;
    } else {
        terminal.finished = true;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_write_completions_are_scoped_to_their_request() -> Result<()> {
        let (send, receive) = mpsc::channel();
        let input = TerminalInput { send };
        let steps = [spec::Step {
            text: "ready".into(),
            input: "yes".into(),
            raw: false,
        }];
        let mut interaction = Interaction::new(&steps);

        interaction.advance("", "ready", &input, true)?;
        interaction.pending = Some(Instant::now());
        interaction.advance("", "ready", &input, true)?;
        let pty_write = receive.recv()?;
        let recovery = input.write(vec![3])?;
        let recovery_write = receive.recv()?;

        recovery_write.completed.send(Ok(()))?;
        assert!(recovery.recv()?.is_ok());
        interaction.advance("", "ready", &input, true)?;
        assert!(interaction.sending());
        assert_eq!(interaction.completed(), 0);

        pty_write.completed.send(Ok(()))?;
        interaction.advance("", "ready", &input, true)?;
        assert!(!interaction.sending());
        assert_eq!(interaction.completed(), 1);
        Ok(())
    }

    #[test]
    fn settling_a_pty_write_completes_its_step() -> Result<()> {
        let (send, receive) = mpsc::channel();
        let input = TerminalInput { send };
        let steps = [spec::Step {
            text: "ready".into(),
            input: "yes".into(),
            raw: false,
        }];
        let mut interaction = Interaction::new(&steps);

        interaction.advance("", "ready", &input, true)?;
        interaction.pending = Some(Instant::now());
        interaction.advance("", "ready", &input, true)?;
        let write = receive.recv()?;
        write.completed.send(Ok(()))?;

        interaction.settle_write(Duration::from_secs(1))?;
        assert!(!interaction.sending());
        assert_eq!(interaction.completed(), 1);
        interaction.check_complete(true, 0, "")?;
        Ok(())
    }
}
