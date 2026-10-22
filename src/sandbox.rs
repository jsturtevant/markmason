use crate::{process, spec};
use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use std::{
    ffi::OsString,
    io::{Read, Write},
    net::{IpAddr, ToSocketAddrs},
    sync::mpsc,
    thread,
    time::Instant,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    #[default]
    None,
    Mxc,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum WorkdirAccess {
    None,
    #[value(name = "ro", alias = "read-only")]
    ReadOnly,
    #[default]
    #[value(name = "rw", alias = "read-write")]
    ReadWrite,
}

impl WorkdirAccess {
    fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::ReadOnly => 1,
            Self::ReadWrite => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliPolicy {
    pub backend: Backend,
    pub workdir: WorkdirAccess,
    pub readonly: Vec<PathBuf>,
    pub readwrite: Vec<PathBuf>,
    pub network_allow: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Policy {
    backend: Backend,
    workdir: WorkdirAccess,
    readonly: Vec<PathBuf>,
    readwrite: Vec<PathBuf>,
    network_allow: Vec<String>,
}

impl Policy {
    pub fn resolve(cli: &CliPolicy, document: Option<&spec::Sandbox>) -> Result<Self> {
        let requested = document.map(|sandbox| sandbox.backend);
        if cli.backend == Backend::Mxc && requested == Some(Backend::None) {
            bail!("document requested sandbox 'none', but the CLI requires 'mxc'");
        }
        let backend = match (cli.backend, requested) {
            (Backend::Mxc, _) | (_, Some(Backend::Mxc)) => Backend::Mxc,
            _ => Backend::None,
        };
        let requested_workdir = document.and_then(|sandbox| sandbox.workdir);
        if requested_workdir.is_some_and(|access| access.rank() > cli.workdir.rank()) {
            bail!(
                "document requested sandbox workdir access '{}', but the CLI permits only '{}'",
                requested_workdir.unwrap().name(),
                cli.workdir.name()
            );
        }
        Ok(Self {
            backend,
            workdir: requested_workdir.unwrap_or(cli.workdir),
            readonly: cli.readonly.clone(),
            readwrite: cli.readwrite.clone(),
            network_allow: cli.network_allow.clone(),
        })
    }
}

impl WorkdirAccess {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReadOnly => "read-only",
            Self::ReadWrite => "read-write",
        }
    }
}

#[derive(Clone)]
pub struct Executor {
    policy: Policy,
    fallback_cwd: PathBuf,
    approved_cwds: Vec<PathBuf>,
    readonly: Vec<PathBuf>,
    readwrite: Vec<PathBuf>,
}

pub struct RunRequest<'a> {
    pub args: &'a [String],
    pub cwd: &'a Path,
    pub env: &'a process::Environment,
    pub input: Option<Vec<u8>>,
    pub timeout: Duration,
    pub scope: &'a process::Scope,
    pub progress: &'a mut dyn FnMut() -> Result<()>,
}

pub struct PtyRequest<'a> {
    pub args: &'a [String],
    pub cwd: &'a Path,
    pub env: &'a process::Environment,
    pub steps: &'a [spec::Step],
    pub timeout: Duration,
    pub scope: &'a process::Scope,
    pub final_command: bool,
    pub progress: &'a mut dyn FnMut() -> Result<()>,
}

impl Executor {
    pub fn new(
        policy: Policy,
        cwd: &Path,
        setup_cwd: &Path,
        home: &Path,
        config: &Path,
    ) -> Result<Self> {
        let cwd = canonical_workdir(cwd)?;
        let setup_cwd = canonical_workdir(setup_cwd)?;
        let mut readonly = canonical_paths(&policy.readonly, "read-only")?;
        let mut readwrite = canonical_paths(&policy.readwrite, "read-write")?;
        let mut candidate_cwds = vec![cwd, setup_cwd];
        candidate_cwds.sort();
        candidate_cwds.dedup();
        for workdir in &candidate_cwds {
            match policy.workdir {
                WorkdirAccess::None => {}
                WorkdirAccess::ReadOnly => readonly.push(workdir.clone()),
                WorkdirAccess::ReadWrite => readwrite.push(workdir.clone()),
            }
        }
        let fallback_cwd = dunce::canonicalize(home)?;
        readwrite.push(fallback_cwd.clone());
        readwrite.push(dunce::canonicalize(config)?);
        normalize_mounts(&mut readonly, &mut readwrite)?;
        let approved_cwds = candidate_cwds
            .into_iter()
            .filter(|cwd| {
                readonly
                    .iter()
                    .chain(&readwrite)
                    .any(|mount| cwd.starts_with(mount))
            })
            .collect();
        Ok(Self {
            policy,
            fallback_cwd,
            approved_cwds,
            readonly,
            readwrite,
        })
    }

    pub fn with_write_path(&self, path: &Path) -> Result<Self> {
        let mut executor = self.clone();
        executor.readwrite.push(dunce::canonicalize(path)?);
        normalize_mounts(&mut executor.readonly, &mut executor.readwrite)?;
        Ok(executor)
    }

    pub fn validate_prerequisite(&self, document: Option<&spec::Sandbox>) -> Result<()> {
        let Some(document) = document else {
            return Ok(());
        };
        if document.backend != self.policy.backend {
            bail!(
                "Markdown setup requests sandbox '{}', but its caller uses '{}'",
                document.backend.name(),
                self.policy.backend.name()
            );
        }
        if let Some(workdir) = document.workdir
            && workdir != self.policy.workdir
        {
            bail!(
                "Markdown setup requests sandbox workdir '{}', but its shared fixture uses '{}'",
                workdir.name(),
                self.policy.workdir.name()
            );
        }
        Ok(())
    }

    pub fn terminal(
        &self,
        args: &[String],
        cwd: &Path,
        env: &process::Environment,
    ) -> Result<process::Terminal> {
        match self.policy.backend {
            Backend::None => process::Terminal::new(args, cwd, env),
            Backend::Mxc => {
                use mxc_sdk::v1::{self, MxcPtySize, SpawnWithPtyOptions};
                let terminal = v1::spawn_with_pty(
                    self.request(args, cwd, env, None)?,
                    SpawnWithPtyOptions {
                        telemetry: None,
                        size: MxcPtySize {
                            rows: 30,
                            cols: 120,
                            pixel_width: 0,
                            pixel_height: 0,
                        },
                    },
                )
                .map_err(mxc_error)?;
                process::Terminal::from_mxc(terminal)
            }
        }
    }

    pub fn run(&self, request: RunRequest<'_>) -> Result<process::Output> {
        match self.policy.backend {
            Backend::None => process::run_with_progress(
                request.args,
                request.cwd,
                request.env,
                request.input,
                request.timeout,
                request.scope,
                request.progress,
            ),
            Backend::Mxc => self.run_mxc(request),
        }
    }

    pub fn pty(&self, request: PtyRequest<'_>) -> Result<(process::Output, usize)> {
        match self.policy.backend {
            Backend::None => process::pty(
                request.args,
                request.cwd,
                request.env,
                request.steps,
                request.timeout,
                process::PtyExecution {
                    scope: request.scope,
                    final_command: request.final_command,
                    progress: request.progress,
                },
            ),
            Backend::Mxc => self.pty_mxc(request),
        }
    }
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Mxc => "mxc",
        }
    }
}

fn canonical_workdir(cwd: &Path) -> Result<PathBuf> {
    dunce::canonicalize(cwd)
        .with_context(|| format!("Cannot resolve sandbox working directory {}", cwd.display()))
}

fn canonical_paths(paths: &[PathBuf], label: &str) -> Result<Vec<PathBuf>> {
    paths
        .iter()
        .map(|path| {
            dunce::canonicalize(path)
                .with_context(|| format!("Cannot resolve sandbox {label} path {}", path.display()))
        })
        .collect()
}

fn normalize_mounts(readonly: &mut Vec<PathBuf>, readwrite: &mut Vec<PathBuf>) -> Result<()> {
    readonly.sort();
    readonly.dedup();
    readwrite.sort();
    readwrite.dedup();
    for read in readonly.iter() {
        for write in readwrite.iter() {
            if read == write || read.starts_with(write) || write.starts_with(read) {
                bail!(
                    "Conflicting sandbox mounts: read-only {} overlaps read-write {}",
                    read.display(),
                    write.display()
                );
            }
        }
    }
    Ok(())
}

impl Executor {
    fn request(
        &self,
        args: &[String],
        cwd: &Path,
        env: &process::Environment,
        timeout: Option<Duration>,
    ) -> Result<mxc_sdk::v1::ContainerRequest> {
        use mxc_sdk::v1::policy::{
            FilesystemPolicy,
            filesystem::{ToolsPolicyOptions, available_tools_policy},
        };
        use mxc_sdk::v1::{ContainerRequest, Containment};

        let timeout_ms = timeout
            .map(|timeout| {
                u32::try_from(timeout.as_millis())
                    .context("Sandbox timeout exceeds MXC's supported range")
            })
            .transpose()?;
        let environment = env
            .iter()
            .map(|(name, value)| {
                Ok((
                    os_string(name, "environment name")?,
                    os_string(value, "environment value")?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let command_cwd = self.command_cwd(cwd)?;
        let mut readonly = self.readonly.clone();
        let mut readwrite = self.readwrite.clone();
        normalize_mounts(&mut readonly, &mut readwrite)?;
        let tools = available_tools_policy(Some(&environment), ToolsPolicyOptions::default());
        let readwrite_paths = display_paths(&readwrite)?;
        let mut readonly_paths = display_paths(&readonly)?;
        let tool_readonly = canonical_policy_paths(tools.readonly_paths)?;
        readonly_paths.extend(tool_readonly.into_iter().filter(|tool| {
            !readwrite_paths
                .iter()
                .any(|write| Path::new(tool).starts_with(write))
        }));
        readonly_paths.sort();
        readonly_paths.dedup();
        let mut readwrite_paths = readwrite_paths;
        readwrite_paths.extend(canonical_policy_paths(tools.readwrite_paths)?);
        readwrite_paths.sort();
        readwrite_paths.dedup();
        Ok(ContainerRequest {
            containment: Containment::Process,
            filesystem: Some(FilesystemPolicy {
                readonly_paths,
                readwrite_paths,
                ..Default::default()
            }),
            network: Some(Self::network_policy(&self.policy.network_allow)?),
            timeout_ms,
            working_directory: Some(path_string(&command_cwd, "working directory")?),
            environment: Some(environment),
            inherit_default_environment: Some(false),
            ..ContainerRequest::new(command_line(args)?)
        })
    }

    fn command_cwd(&self, cwd: &Path) -> Result<PathBuf> {
        let requested = std::path::absolute(cwd).with_context(|| {
            format!("Cannot resolve command working directory {}", cwd.display())
        })?;
        let cwd = dunce::canonicalize(cwd).with_context(|| {
            format!("Cannot resolve command working directory {}", cwd.display())
        })?;
        if self.approved_cwds.contains(&requested) && cwd != requested {
            bail!(
                "Sandbox command working directory resolves outside approved permissions: {}",
                cwd.display()
            );
        }
        if self.approved_cwds.contains(&cwd) {
            Ok(cwd)
        } else if self.policy.workdir == WorkdirAccess::None {
            Ok(self.fallback_cwd.clone())
        } else {
            bail!(
                "Sandbox command working directory resolves outside approved permissions: {}",
                cwd.display()
            )
        }
    }

    fn network_policy(entries: &[String]) -> Result<mxc_sdk::v1::policy::NetworkPolicy> {
        use mxc_sdk::v1::policy::{
            NetworkAction, NetworkEgressPolicy, NetworkIngressPolicy, NetworkPeerPolicy,
            NetworkPortPolicy, NetworkProtocol, NetworkRulePolicy,
        };

        let mut allow = Vec::new();
        for entry in entries {
            let (host, port) = Self::split_destination(entry)?;
            let addresses = (host.as_str(), port.unwrap_or(0))
                .to_socket_addrs()
                .with_context(|| format!("Cannot resolve sandbox network destination {entry}"))?
                .map(|address| address.ip())
                .collect::<std::collections::BTreeSet<_>>();
            if addresses.is_empty() {
                bail!("Sandbox network destination resolved to no addresses: {entry}");
            }
            allow.push(NetworkRulePolicy {
                to: Some(
                    addresses
                        .into_iter()
                        .map(|address| NetworkPeerPolicy::new(Self::host_cidr(address)))
                        .collect(),
                ),
                ports: Some(vec![NetworkPortPolicy {
                    protocol: Some(NetworkProtocol::Tcp),
                    port,
                    end_port: None,
                }]),
            });
        }
        Ok(mxc_sdk::v1::policy::NetworkPolicy {
            egress: Some(NetworkEgressPolicy {
                default: Some(NetworkAction::Deny),
                allow: (!allow.is_empty()).then_some(allow),
                deny: None,
            }),
            ingress: Some(NetworkIngressPolicy {
                default: Some(NetworkAction::Deny),
                host_loopback: Some(NetworkAction::Deny),
            }),
            runtime_config: None,
        })
    }

    fn split_destination(entry: &str) -> Result<(String, Option<u16>)> {
        if let Some(host) = entry.strip_prefix('[')
            && let Some((host, suffix)) = host.split_once(']')
        {
            let port = if suffix.is_empty() {
                None
            } else if let Some(port) = suffix.strip_prefix(':') {
                Some(port.parse().context("Invalid sandbox network port")?)
            } else {
                bail!("Invalid sandbox network destination {entry}: expected ':' after ']'");
            };
            return Ok((host.to_owned(), port));
        }
        if let Some((host, port)) = entry.rsplit_once(':')
            && !host.contains(':')
        {
            return Ok((
                host.to_owned(),
                Some(port.parse().context("Invalid sandbox network port")?),
            ));
        }
        Ok((entry.to_owned(), None))
    }

    fn host_cidr(address: IpAddr) -> String {
        match address {
            IpAddr::V4(address) => format!("{address}/32"),
            IpAddr::V6(address) => format!("{address}/128"),
        }
    }

    fn run_mxc(&self, request: RunRequest<'_>) -> Result<process::Output> {
        use mxc_sdk::v1::{self, WaitResult};

        let RunRequest {
            args,
            cwd,
            env,
            input,
            timeout,
            scope: _,
            progress,
        } = request;
        crate::cancellation::check()?;
        let mut child = v1::spawn(
            self.request(args, cwd, env, Some(timeout))?,
            Default::default(),
        )
        .map_err(mxc_error)?;
        let stdout_closer = child.stdout_closer();
        let stderr_closer = child.stderr_closer();
        let stdout = read_stream(child.take_stdout());
        let stderr = read_stream(child.take_stderr());
        let mut stdin = child.take_stdin();
        let (input_tx, input_rx) = mpsc::channel();
        thread::spawn(move || {
            let result = if let (Some(mut stdin), Some(input)) = (stdin.take(), input) {
                stdin.write_all(&input)
            } else {
                Ok(())
            };
            drop(stdin);
            let _ = input_tx.send(result);
        });
        let started = Instant::now();
        let mut cancelled = None;
        let outcome = loop {
            if let Err(error) = crate::cancellation::check() {
                child.kill().context("Failed to cancel MXC command")?;
                cancelled = Some(error);
            }
            progress()?;
            if let Some(code) = child.try_wait()? {
                break WaitResult::Exited(code);
            }
            if started.elapsed() >= timeout {
                child.kill_for_timeout()?;
                break WaitResult::TimedOut;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let waited = child.wait()?;
        let warnings = child.warnings();
        if matches!(outcome, WaitResult::TimedOut) || cancelled.is_some() {
            if let Some(closer) = &stdout_closer {
                closer.close();
            }
            if let Some(closer) = &stderr_closer {
                closer.close();
            }
        }
        let stdout =
            match collect_stream(stdout, stdout_closer, started, timeout, progress, "stdout") {
                Ok(stdout) => stdout,
                Err(error) => {
                    if let Some(closer) = stderr_closer {
                        closer.close();
                    }
                    return Err(error);
                }
            };
        let stderr = collect_stream(stderr, stderr_closer, started, timeout, progress, "stderr")?;
        if let Ok(result) = input_rx.recv_timeout(Duration::from_secs(1))
            && let Err(error) = result
            && error.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(error).context("Failed writing sandbox stdin");
        }
        if let Some(error) = cancelled {
            return Err(error);
        }
        let outcome = if matches!(waited, WaitResult::TimedOut) {
            WaitResult::TimedOut
        } else {
            outcome
        };
        if matches!(outcome, WaitResult::TimedOut) {
            bail!(
                "Command timed out after {}ms: {}\nOutput so far:\n{}{}",
                timeout.as_millis(),
                args.join(" "),
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        let code = match outcome {
            WaitResult::Exited(code) => code,
            WaitResult::TimedOut => unreachable!(),
        };
        let mut text = String::from_utf8_lossy(&stdout).into_owned();
        if code != 0 {
            let stderr = String::from_utf8_lossy(&stderr);
            text.push_str(&stderr);
            if let Some(guidance) = policy_guidance(&stderr) {
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(guidance);
                text.push('\n');
            }
            if !warnings.is_empty() {
                text.push_str("\nSandbox warnings:\n");
                text.push_str(&warnings.join("\n"));
                text.push('\n');
            }

            fn policy_guidance(stderr: &str) -> Option<&'static str> {
                let lower = stderr.to_lowercase();
                if [
                    "network is unreachable",
                    "connection refused",
                    "connection timed out",
                    "could not resolve",
                    "name or service not known",
                ]
                .iter()
                .any(|message| lower.contains(message))
                {
                    return Some(
                        "Sandbox network access may be denied; permit the destination with \
                         --sandbox-net-allow HOST[:PORT].",
                    );
                }
                if [
                    "permission denied",
                    "operation not permitted",
                    "read-only file system",
                    "no such file",
                    "cannot find the path",
                ]
                .iter()
                .any(|message| lower.contains(message))
                {
                    return Some(
                        "Sandbox filesystem access may be denied; permit the path with \
                         --sandbox-read PATH or --sandbox-write PATH.",
                    );
                }
                None
            }
        }
        Ok(process::Output { text, code })
    }

    fn pty_mxc(&self, request: PtyRequest<'_>) -> Result<(process::Output, usize)> {
        use mxc_sdk::v1::{self, MxcPtySize, SpawnWithPtyOptions};

        let PtyRequest {
            args,
            cwd,
            env,
            steps,
            timeout,
            scope,
            final_command,
            progress,
        } = request;
        crate::cancellation::check()?;
        let terminal = v1::spawn_with_pty(
            self.request(args, cwd, env, Some(timeout))?,
            SpawnWithPtyOptions {
                telemetry: None,
                size: MxcPtySize {
                    rows: 30,
                    cols: 120,
                    pixel_width: 0,
                    pixel_height: 0,
                },
            },
        )
        .map_err(mxc_error)?;
        let (mut output, completed) = process::pty_terminal(
            process::Terminal::from_mxc(terminal)?,
            steps,
            timeout,
            scope,
            final_command,
            progress,
        )?;
        if output.code == -1 && output.text.contains("^C") {
            output.code = 130;
        }
        Ok((output, completed))
    }
}

fn read_stream(stream: Option<Box<dyn Read + Send>>) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = if let Some(mut stream) = stream {
            stream.read_to_end(&mut bytes).map(|_| bytes)
        } else {
            Ok(bytes)
        };
        let _ = send.send(result);
    });
    receive
}

fn collect_stream(
    stream: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    closer: Option<mxc_sdk::v1::StreamCloser>,
    started: Instant,
    timeout: Duration,
    progress: &mut dyn FnMut() -> Result<()>,
    label: &str,
) -> Result<Vec<u8>> {
    loop {
        if let Err(error) = crate::cancellation::check() {
            if let Some(closer) = closer {
                closer.close();
            }
            return Err(error);
        }
        progress()?;
        match stream.recv_timeout(Duration::from_millis(10)) {
            Ok(result) => return result.context("Failed reading sandbox output"),
            Err(mpsc::RecvTimeoutError::Timeout) if started.elapsed() < timeout => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(closer) = closer {
                    closer.close();
                }
                bail!("Timed out draining sandbox {label}");
            }
            Err(error) => bail!("Sandbox {label} reader stopped: {error}"),
        }
    }
}

fn command_line(args: &[String]) -> Result<String> {
    if args.is_empty() {
        bail!("Empty sandbox command");
    }
    #[cfg(windows)]
    {
        Ok(args
            .iter()
            .map(|arg| windows_quote(arg))
            .collect::<Vec<_>>()
            .join(" "))
    }
    #[cfg(not(windows))]
    {
        Ok(args
            .iter()
            .map(|arg| crate::shell::quote(arg))
            .collect::<Vec<_>>()
            .join(" "))
    }
}

#[cfg(windows)]
fn windows_quote(argument: &str) -> String {
    if !argument.is_empty() && !argument.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return argument.to_owned();
    }
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for ch in argument.chars() {
        if ch == '\\' {
            slashes += 1;
        } else if ch == '"' {
            quoted.push_str(&"\\".repeat(slashes * 2 + 1));
            quoted.push('"');
            slashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(slashes));
            slashes = 0;
            quoted.push(ch);
        }
    }
    quoted.push_str(&"\\".repeat(slashes * 2));
    quoted.push('"');
    quoted
}

fn os_string(value: &OsString, label: &str) -> Result<String> {
    value
        .to_str()
        .map(str::to_owned)
        .with_context(|| format!("Sandbox {label} is not valid Unicode"))
}

fn display_paths(paths: &[PathBuf]) -> Result<Vec<String>> {
    paths
        .iter()
        .map(|path| path_string(path, "mount path"))
        .collect()
}

fn path_string(path: &Path, label: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("Sandbox {label} is not valid Unicode: {}", path.display()))
}

fn canonical_policy_paths(paths: Vec<String>) -> Result<Vec<String>> {
    paths
        .into_iter()
        .map(|path| {
            let original = PathBuf::from(&path);
            let resolved = dunce::canonicalize(&original).unwrap_or(original);
            resolved
                .into_os_string()
                .into_string()
                .map_err(|_| anyhow::anyhow!("Sandbox tool path is not valid Unicode: {path:?}"))
        })
        .collect()
}

fn mxc_error(error: mxc_sdk::v1::Error) -> anyhow::Error {
    let mut message = format!("MXC sandbox error: {}", error.message);
    if let Some(operation) = error.operation {
        message.push_str(&format!("\n  operation: {operation}"));
    }
    if let Some(code) = error.native_code {
        message.push_str(&format!("\n  native code: {code}"));
    }
    if let Some(remediation) = error.remediation {
        message.push_str(&format!("\n  remediation: {remediation}"));
    }
    anyhow::anyhow!(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_cannot_relax_cli_policy() {
        let cli = CliPolicy {
            backend: Backend::Mxc,
            workdir: WorkdirAccess::ReadOnly,
            readonly: Vec::new(),
            readwrite: Vec::new(),
            network_allow: Vec::new(),
        };
        let document = spec::Sandbox {
            backend: Backend::None,
            workdir: Some(WorkdirAccess::ReadWrite),
        };
        assert!(Policy::resolve(&cli, Some(&document)).is_err());
    }

    #[test]
    fn conflicting_mounts_are_rejected() {
        let root = PathBuf::from("/tmp/project");
        let mut readonly = vec![root.join("fixtures")];
        let mut readwrite = vec![root];
        assert!(normalize_mounts(&mut readonly, &mut readwrite).is_err());
    }

    #[test]
    fn malformed_ipv6_network_suffix_is_rejected() {
        assert!(Executor::split_destination("[::1]443").is_err());
        assert_eq!(
            Executor::split_destination("[::1]:443").unwrap(),
            ("::1".to_owned(), Some(443))
        );
    }

    #[test]
    fn network_permissions_are_always_tcp_scoped() {
        use mxc_sdk::v1::policy::NetworkProtocol;

        for (entry, expected_port) in [("127.0.0.1", None), ("127.0.0.1:443", Some(443))] {
            let policy = Executor::network_policy(&[entry.to_owned()]).unwrap();
            let egress = policy.egress.unwrap();
            let allow = egress.allow.unwrap();
            let port = &allow[0].ports.as_ref().unwrap()[0];
            assert_eq!(port.protocol, Some(NetworkProtocol::Tcp));
            assert_eq!(port.port, expected_port);
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_mount_paths_are_rejected() {
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(b"grant-\xff".to_vec()));
        assert!(display_paths(&[path]).is_err());
    }
}
