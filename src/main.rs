mod cancellation;
mod matching;
mod presentation;
mod process;
mod shell;
mod spec;
mod step;
#[cfg(windows)]
mod windows;

use anyhow::{Context, Result, bail};
use clap::Parser;
use regex::Regex;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    env, fs,
    io::{self, Write},
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tempfile::{Builder, TempDir};

#[derive(Debug, Clone, Parser)]
#[command(name = "mm", version, about)]
struct Options {
    /// Markdown specification file, or directory searched recursively.
    #[arg(default_value = "tests/cli")]
    spec_dir: PathBuf,
    /// Working directory for specs without clone frontmatter.
    #[arg(long, default_value = ".")]
    cwd: PathBuf,
    /// Name of the isolated application-home environment variable and placeholder.
    #[arg(long, default_value = "E2E_HOME")]
    home_env: String,
    /// Prefix for FILE_PATTERN, FILE_EXCLUDE, TAGS and TAGS_EXCLUDE environment filters.
    #[arg(long, default_value = "E2E_")]
    env_prefix: String,
    #[arg(long)]
    file_pattern: Option<String>,
    #[arg(long)]
    file_exclude: Option<String>,
    #[arg(long)]
    tags: Option<String>,
    #[arg(long)]
    tags_exclude: Option<String>,
    /// Regex matching a file name or command.
    #[arg(long = "test-name-pattern")]
    name_patterns: Vec<String>,
    /// Default per-command timeout in seconds; frontmatter takes precedence.
    #[arg(long, default_value = "30", value_parser = positive_seconds)]
    timeout: f64,
    /// Print commands without executing them or creating fixtures.
    #[arg(long)]
    list: bool,
    /// Walk through commands; pick a file unless a file path or --file-pattern is set.
    #[arg(long, conflicts_with = "list")]
    step: bool,
    /// Stop scheduling documents after the first failed document finishes cleanup.
    #[arg(long, conflicts_with_all = ["list", "step"])]
    fail_fast: bool,
    /// Run Markdown files concurrently; omit N to use available logical CPUs.
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "auto",
        value_parser = parallelism,
        conflicts_with = "step"
    )]
    parallel: Option<Parallelism>,
    #[arg(long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy)]
enum Parallelism {
    Auto,
    Jobs(usize),
}

fn parallelism(text: &str) -> std::result::Result<Parallelism, String> {
    if text == "auto" {
        return Ok(Parallelism::Auto);
    }
    let jobs = text
        .parse::<usize>()
        .map_err(|_| "parallel workers must be 'auto' or a positive integer".to_owned())?;
    if jobs == 0 {
        return Err("parallel workers must be greater than zero".to_owned());
    }
    if jobs > 256 {
        return Err("parallel workers must not exceed 256".to_owned());
    }
    Ok(Parallelism::Jobs(jobs))
}

fn positive_seconds(text: &str) -> std::result::Result<f64, String> {
    let value = text.parse::<f64>().map_err(|e| e.to_string())?;
    if !value.is_finite() || value <= 0.0 || Duration::try_from_secs_f64(value).is_err() {
        return Err("timeout must be positive, finite seconds".to_owned());
    }
    Ok(value)
}

fn regex(value: Option<String>, name: &str) -> Result<Option<Regex>> {
    value
        .filter(|s| !s.is_empty())
        .map(|s| Regex::new(&s).with_context(|| format!("Invalid {name} regex")))
        .transpose()
}

fn tags(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn discover(dir: &Path, base: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Cannot read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            discover(&path, base, files)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(
                path.strip_prefix(base)?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

fn resolve(command: &str, dir: &Path) -> Result<Vec<String>> {
    let mut args = spec::split_command(command);
    let name = args
        .first()
        .context("Empty command in pipeline stage")?
        .clone();
    if name == "node" && args.get(1).is_some_and(|arg| arg.starts_with("./")) {
        args[1] = dir.join(&args[1]).to_string_lossy().into_owned();
    }
    Ok(args)
}

fn resolve_setup(command: &str, dir: &Path) -> Result<Vec<String>> {
    let mut args = spec::split_command(command);
    if args.first().is_none_or(|arg| arg.is_empty()) {
        bail!("Empty setup command");
    }
    for arg in &mut args {
        let path = Path::new(arg);
        if path.is_absolute() || arg.is_empty() || arg.starts_with('-') {
            continue;
        }
        let explicit_relative = matches!(
            path.components().next(),
            Some(std::path::Component::CurDir | std::path::Component::ParentDir)
        );
        let resolved = dir.join(path);
        if explicit_relative || resolved.is_file() {
            *arg = resolved.to_string_lossy().into_owned();
        }
    }
    Ok(args)
}

struct Fixture {
    shell: RefCell<Option<shell::Session>>,
    processes: process::Scope,
    _home: TempDir,
    _config: TempDir,
    _repo: Option<TempDir>,
    cwd: PathBuf,
    env: process::Environment,
    vars: Vec<(String, String)>,
}

fn gh_config_dir() -> Result<PathBuf> {
    let nonempty = |key| env::var_os(key).filter(|value| !value.is_empty());
    if let Some(dir) = nonempty("GH_CONFIG_DIR") {
        return Ok(dir.into());
    }
    if let Some(dir) = nonempty("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(dir).join("gh"));
    }
    #[cfg(windows)]
    if let Some(dir) = nonempty("APPDATA") {
        return Ok(PathBuf::from(dir).join("GitHub CLI"));
    }
    let home = nonempty("HOME").or_else(|| nonempty("USERPROFILE"));
    Ok(
        PathBuf::from(home.context("Cannot resolve home directory for gh configuration")?)
            .join(".config/gh"),
    )
}

impl Fixture {
    fn new(
        options: &Options,
        spec: &spec::Spec,
        gh_config: &Path,
        mut env: process::Environment,
    ) -> Result<Self> {
        let home = Builder::new().prefix("markmason-home-").tempdir()?;
        let config = Builder::new().prefix("markmason-xdg-").tempdir()?;
        let processes = process::Scope::default();
        for (key, value) in [
            ("NO_COLOR", "1"),
            ("FORCE_COLOR", "0"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GH_PROMPT_DISABLED", "1"),
        ] {
            process::set_env(&mut env, key, value);
        }
        process::set_env(&mut env, "GH_CONFIG_DIR", gh_config.as_os_str());
        process::set_env(&mut env, &options.home_env, home.path().as_os_str());
        process::set_env(&mut env, "XDG_CONFIG_HOME", config.path().as_os_str());
        #[cfg(windows)]
        for name in ["APPDATA", "LOCALAPPDATA"] {
            process::set_env(&mut env, name, config.path().as_os_str());
        }
        let mut cwd = dunce::canonicalize(&options.cwd)?;
        let repo = if let Some(owner_repo) = &spec.clone {
            let auth = process::run(
                &["gh".into(), "auth".into(), "status".into()],
                &cwd,
                &env,
                None,
                Duration::from_secs(30),
                &processes,
            )
            .context("gh not installed or unavailable; install GitHub CLI to clone fixtures")?;
            if auth.code != 0 {
                bail!("gh not authenticated. Run 'gh auth login'.\n{}", auth.text);
            }
            let repo = Builder::new().prefix("markmason-repo-").tempdir()?;
            let cloned = process::run(
                &[
                    "gh".into(),
                    "repo".into(),
                    "clone".into(),
                    owner_repo.clone(),
                    repo.path().to_string_lossy().into_owned(),
                ],
                &cwd,
                &env,
                None,
                Duration::from_secs(30),
                &processes,
            )?;
            if cloned.code != 0 {
                bail!("Failed to clone {owner_repo}:\n{}", cloned.text);
            }
            cwd = repo.path().to_owned();
            Some(repo)
        } else {
            None
        };
        let vars = vec![
            (
                format!("${}", options.home_env),
                home.path().to_string_lossy().into_owned(),
            ),
            ("$REPO_ROOT".into(), cwd.to_string_lossy().into_owned()),
            (
                "$PROJECT_NAME".into(),
                cwd.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "$XDG_CONFIG_HOME".into(),
                config.path().to_string_lossy().into_owned(),
            ),
        ];
        let shell = if spec.shell.is_some() {
            let mut shell_env = env.clone();
            for (key, value) in &vars {
                process::set_env(
                    &mut shell_env,
                    key.trim_start_matches('$'),
                    shell::path(Path::new(value)),
                );
            }
            let mut shell = shell::Session::new(
                &cwd,
                &shell_env,
                spec.timeout
                    .unwrap_or(Duration::from_secs_f64(options.timeout)),
            )?;
            if spec.cases.iter().any(|case| case.always) {
                shell.enable_cleanup()?;
            }
            Some(shell)
        } else {
            None
        };
        Ok(Self {
            shell: RefCell::new(shell),
            processes,
            _home: home,
            _config: config,
            _repo: repo,
            cwd,
            env,
            vars,
        })
    }

    fn substitute(&self, text: &str) -> String {
        self.vars
            .iter()
            .fold(text.to_owned(), |text, (key, value)| {
                text.replace(key, value)
            })
    }

    fn close(self) -> Result<()> {
        if let Some(shell) = self.shell.into_inner() {
            shell.close()?;
        }
        self.processes.close()?;
        if let Some(repo) = self._repo {
            repo.close().context("Failed to clean up cloned fixture")?;
        }
        self._home
            .close()
            .context("Failed to clean up application home")?;
        self._config
            .close()
            .context("Failed to clean up isolated configuration")?;
        Ok(())
    }
}

fn execute(
    case: &spec::Case,
    fixture: &Fixture,
    dir: &Path,
    timeout: Duration,
    verbose: bool,
    diagnostics: Option<&mut String>,
    mut stepping: Option<&mut step::Session>,
) -> Result<bool> {
    let shell_mode = fixture.shell.borrow().is_some();
    let commands = if shell_mode {
        shell::commands(&case.command)?
    } else {
        case.command.lines().map(str::to_owned).collect()
    };
    let expected = case.expected.as_ref().map(|text| fixture.substitute(text));
    if case.matching == matching::Mode::Regex
        && let Some(expected) = &expected
    {
        Regex::new(expected)
            .context("Invalid expected-regex pattern after variable substitution")?;
    }
    if let Some(session) = stepping.as_deref_mut() {
        session.prepare(case, &commands, expected.as_deref())?;
        if session.stopped() {
            return Ok(false);
        }
    }
    let mut output = process::Output {
        text: String::new(),
        code: 0,
    };
    let mut stdin = case.stdin.as_ref().map(|s| s.as_bytes().to_vec());
    let mut step_index = 0;
    let mut shell_block = case.pty.as_deref().map(shell::Block::new);
    for (index, command) in commands.iter().enumerate() {
        let command = if shell_mode {
            command.clone()
        } else {
            fixture.substitute(command)
        };
        if let Some(session) = stepping.as_deref_mut() {
            session.preview(index + 1)?;
        }
        let result = (|| -> Result<process::Output> {
            let mut progress = || {
                if let Some(session) = stepping.as_deref_mut() {
                    session.refresh_timer()?;
                }
                Ok(())
            };
            if let Some(shell) = fixture.shell.borrow_mut().as_mut() {
                shell.run_with_progress(
                    &command,
                    stdin.take(),
                    shell_block.as_mut(),
                    timeout,
                    index + 1 == commands.len(),
                    &mut progress,
                )
            } else if let Some(steps) = &case.pty {
                let (result, completed) = process::pty(
                    &resolve(&command, dir)?,
                    &fixture.cwd,
                    &fixture.env,
                    &steps[step_index..],
                    timeout,
                    process::PtyExecution {
                        scope: &fixture.processes,
                        final_command: index + 1 == commands.len(),
                        progress: &mut progress,
                    },
                )?;
                step_index += completed;
                Ok(result)
            } else {
                let stages = spec::split_pipeline(&command);
                if stages.is_empty() {
                    bail!("Empty pipeline: no commands to execute");
                }
                let mut input = stdin.take();
                let mut output = None;
                for stage in stages {
                    let result = process::run_with_progress(
                        &resolve(&stage, dir)?,
                        &fixture.cwd,
                        &fixture.env,
                        input,
                        timeout,
                        &fixture.processes,
                        &mut progress,
                    )?;
                    if result.code != 0 {
                        output = Some(result);
                        break;
                    }
                    input = Some(result.text.as_bytes().to_vec());
                    output = Some(result);
                }
                output.context("Pipeline produced no output")
            }
        })();
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                if let Some(session) = stepping.as_deref_mut() {
                    session.inspect(
                        &output.text,
                        None,
                        step::Status::ExecutionError(&error),
                        shell_mode,
                    )?;
                }
                return Err(error);
            }
        };
        // ConPTY can omit the final newline; separate distinct terminal sessions.
        if !shell_mode
            && case.pty.is_some()
            && !output.text.is_empty()
            && !output.text.ends_with('\n')
            && !result.text.is_empty()
        {
            output.text.push('\n');
        }
        output.text.push_str(&result.text);
        output.code = result.code;
        if let Some(session) = stepping.as_deref_mut() {
            let complete = index + 1 == commands.len() || output.code != 0;
            let verdict = if complete {
                check_output(case, expected.as_deref(), &output, false, false, None)
            } else {
                Ok(())
            };
            let status = match &verdict {
                Err(error) => step::Status::Failed(error),
                Ok(()) if complete => step::Status::Passed,
                Ok(()) => step::Status::Progress,
            };
            session.inspect(
                &output.text,
                Some(output.code),
                status,
                shell_mode && verdict.is_err(),
            )?;
            if complete {
                return verdict.map(|()| true);
            }
            if session.stopped() {
                return Ok(false);
            }
        }
        if output.code != 0 {
            break;
        }
    }
    check_output(
        case,
        expected.as_deref(),
        &output,
        verbose,
        true,
        diagnostics,
    )
    .map(|()| true)
}

fn check_output(
    case: &spec::Case,
    expected: Option<&str>,
    output: &process::Output,
    verbose: bool,
    diagnostics: bool,
    mut buffered: Option<&mut String>,
) -> Result<()> {
    if verbose {
        emit_diagnostic(&mut buffered, &format!("ACTUAL:\n{}\n", output.text));
    }
    if let Some(expected) = expected {
        let mode = case.output_mode();
        let result = matching::check(&output.text, expected, mode);
        if verbose {
            if matches!(mode, matching::Mode::Substring | matching::Mode::Regex) {
                emit_diagnostic(
                    &mut buffered,
                    &format!(
                        "ACTUAL:\n{}\nEXPECTED ({mode:?}):\n{expected}\n",
                        output.text
                    ),
                );
            } else {
                emit_diagnostic(
                    &mut buffered,
                    &format!(
                        "ACTUAL:\n{}\nDIFF (expected vs actual):\n{}\n",
                        output.text,
                        matching::diff(&output.text, expected)
                    ),
                );
            }
        }
        if let Err(error) = result {
            if diagnostics && !verbose {
                if matches!(mode, matching::Mode::Substring | matching::Mode::Regex) {
                    bail!(
                        "{error:#}\nACTUAL:\n{}\nEXPECTED ({mode:?}):\n{expected}",
                        output.text
                    );
                }
                bail!(
                    "{error:#}\nACTUAL:\n{}\nDIFF (expected vs actual):\n{}",
                    output.text,
                    matching::diff(&output.text, expected)
                );
            }
            return Err(error);
        }
    }
    if output.code != case.exit_code {
        bail!(
            "Command exited with code {} (expected exit {}).\nOutput:\n{}",
            output.code,
            case.exit_code,
            output.text
        );
    }
    Ok(())
}

fn emit_diagnostic(buffered: &mut Option<&mut String>, text: &str) {
    if let Some(buffer) = buffered.as_deref_mut() {
        buffer.push_str(text);
    } else {
        print!("{text}");
    }
}

fn run_setup(
    spec: &spec::Spec,
    dir: &Path,
    fixture: &Fixture,
    setup_cwd: &Path,
    timeout: Duration,
    output: (bool, Option<&mut String>),
    state: &mut Setup,
) -> Result<()> {
    let (verbose, mut diagnostics) = output;
    let Some(setup) = &spec.setup else {
        return Ok(());
    };
    let args = resolve_setup(setup, dir)?;
    let executable = Path::new(&args[0]);
    let markdown = args.len() == 2
        && executable
            .file_name()
            .is_some_and(|name| name == "mm" || name == "mm.exe")
        && Path::new(&args[1])
            .extension()
            .is_some_and(|ext| ext == "md");
    if markdown {
        let path = dunce::canonicalize(dir.join(&args[1]))
            .with_context(|| format!("Cannot find Markdown setup {}", args[1]))?;
        if state.chain.contains(&path) {
            bail!("Circular Markdown setup: {}", path.display());
        }
        let inherited_shell = fixture
            .shell
            .borrow()
            .is_some()
            .then_some(spec::Shell::Bash);
        let setup_spec = spec::parse_with_shell(&fs::read_to_string(&path)?, inherited_shell)
            .with_context(|| format!("Invalid Markdown setup {}", path.display()))?;
        if setup_spec.shell.is_some() && inherited_shell.is_none() {
            bail!("Markdown setup uses shell: bash; specify shell: bash in the calling file");
        }
        if setup_spec.clone.is_some() {
            bail!(
                "Markdown setup {} cannot use clone; put clone in the calling file",
                path.display()
            );
        }
        if !setup_spec.cases.iter().any(|case| !case.always) {
            bail!("No test cases found in Markdown setup {}", path.display());
        }
        let dir = path
            .parent()
            .context("Markdown setup has no parent directory")?;
        let timeout = setup_spec.timeout.unwrap_or(timeout);
        if setup_spec.cases.iter().any(|case| case.always)
            && let Some(shell) = fixture.shell.borrow_mut().as_mut()
        {
            shell.enable_cleanup()?;
        }
        state.chain.push(path.clone());
        run_setup(
            &setup_spec,
            dir,
            fixture,
            &fixture.cwd,
            timeout,
            (verbose, diagnostics.as_deref_mut()),
            state,
        )
        .with_context(|| format!("Markdown setup {} failed", path.display()))?;
        for case in setup_spec.cases.iter().filter(|case| !case.always) {
            cancellation::check()?;
            execute(
                case,
                fixture,
                dir,
                timeout,
                verbose,
                diagnostics.as_deref_mut(),
                None,
            )
            .with_context(|| {
                format!("Markdown setup {} failed: {}", path.display(), case.command)
            })?;
        }
        defer(
            &setup_spec,
            dir,
            timeout,
            &path.display().to_string(),
            0,
            &mut state.deferred,
        );
        state.chain.pop();
    } else {
        let result = if let Some(shell) = fixture.shell.borrow_mut().as_mut() {
            let command = args
                .iter()
                .map(|arg| {
                    let text = if Path::new(arg).is_absolute() {
                        shell::path(Path::new(arg))
                    } else {
                        arg.clone()
                    };
                    shell::quote(&text)
                })
                .collect::<Vec<_>>()
                .join(" ");
            shell.run(&command, None, None, timeout, true)
        } else {
            process::run(
                &args,
                setup_cwd,
                &fixture.env,
                None,
                timeout,
                &fixture.processes,
            )
        }
        .with_context(|| format!("Setup command failed: {setup}"))?;
        if result.code != 0 {
            bail!("Setup command failed: {setup}\n{}", result.text);
        }
        if verbose {
            emit_diagnostic(
                &mut diagnostics,
                &format!("Setup ({setup}):\n{}\n", result.text),
            );
        }
    }
    Ok(())
}

struct Deferred {
    case: spec::Case,
    dir: PathBuf,
    timeout: Duration,
    file: String,
    position: usize,
    total: usize,
}

struct Setup {
    chain: Vec<PathBuf>,
    deferred: Vec<Deferred>,
}

fn defer(
    spec: &spec::Spec,
    dir: &Path,
    timeout: Duration,
    file: &str,
    ordinary_blocks: usize,
    deferred: &mut Vec<Deferred>,
) {
    let always: Vec<_> = spec.cases.iter().filter(|case| case.always).collect();
    let total = ordinary_blocks + always.len();
    deferred.extend(
        always
            .into_iter()
            .enumerate()
            .map(|(index, case)| Deferred {
                case: case.clone(),
                dir: dir.to_owned(),
                timeout,
                file: file.to_owned(),
                position: ordinary_blocks + index + 1,
                total,
            }),
    );
}

fn presented_block(case: &spec::Case, position: usize, total: usize) -> presentation::Block {
    presentation::Block {
        position,
        total,
        heading: presentation::heading(case.title.as_deref(), &case.command),
        command: case.command.clone(),
        deferred: case.always,
    }
}

fn run_deferred(
    deferred: &[Deferred],
    fixture: &Fixture,
    verbose: bool,
    document: &mut presentation::Document,
    presenter: &mut presentation::Presenter,
) -> Result<()> {
    for block in deferred {
        let _cleanup = cancellation::Cleanup::new();
        let presented = presented_block(&block.case, block.position, block.total);
        let progress = presenter.block_start(&presented)?;
        let mut diagnostics = String::new();
        match execute(
            &block.case,
            fixture,
            &block.dir,
            block.timeout,
            verbose,
            Some(&mut diagnostics),
            None,
        ) {
            Ok(true) => {
                document.passed += 1;
                presenter.block_pass(progress, &presented, &diagnostics)?;
            }
            Ok(false) => unreachable!("Deferred blocks never use walkthrough input"),
            Err(error) => {
                document.failed += 1;
                document.failed_headings.push(presented.heading.clone());
                presenter.block_fail(progress, &block.file, &presented, &error, &diagnostics)?;
            }
        }
    }
    Ok(())
}

struct FileRun {
    document: presentation::Document,
    output: String,
}

fn run_unattended_document(
    options: &Options,
    root: &Path,
    gh_config: &Path,
    selected: (String, spec::Spec, Vec<usize>),
    verbose: bool,
    buffered: bool,
) -> Result<FileRun> {
    let (file, spec, indices) = selected;
    let mut presenter = if buffered {
        presentation::Presenter::buffered()
    } else {
        presentation::Presenter::new()
    };
    let listed_total = indices.len() + spec.cases.iter().filter(|case| case.always).count();
    presenter.document_start(&file)?;
    if indices.is_empty() {
        let error = anyhow::anyhow!(
            "No test cases found. Expected a fenced markmason command block without always"
        );
        presenter.setup_fail(&file, &error)?;
        let document = presentation::Document {
            file,
            total: listed_total,
            passed: 0,
            failed: 0,
            errors: 1,
            failed_headings: Vec::new(),
            cancelled: false,
            not_run: false,
            duration: Duration::ZERO,
        };
        presenter.document_finish(&document)?;
        return Ok(FileRun {
            document,
            output: presenter.into_buffer(),
        });
    }
    let started = std::time::Instant::now();
    let mut document = presentation::Document {
        file: file.clone(),
        total: listed_total,
        passed: 0,
        failed: 0,
        errors: 0,
        failed_headings: Vec::new(),
        cancelled: false,
        not_run: false,
        duration: Duration::ZERO,
    };
    let timeout = spec
        .timeout
        .unwrap_or(Duration::from_secs_f64(options.timeout));
    let dir = root
        .join(&file)
        .parent()
        .context("Spec has no parent directory")?
        .to_owned();
    let fixture = (|| -> Result<Fixture> {
        let mut inputs = Vec::new();
        collect_inputs(&spec, &dir, &mut vec![root.join(&file)], &mut inputs)?;
        let env = input_environment(&inputs, &options.home_env, &file, None)?
            .context("Unattended input collection unexpectedly stopped")?;
        cancellation::check()?;
        Fixture::new(options, &spec, gh_config, env)
    })();
    let fixture = match fixture {
        Ok(fixture) => fixture,
        Err(error) => {
            document.cancelled = cancellation::signal().is_some();
            document.duration = started.elapsed();
            if document.cancelled {
                presenter.document_cancel(&file, "Setup", &error)?;
            } else {
                document.errors += 1;
                presenter.setup_fail(&file, &error)?;
            }
            presenter.document_finish(&document)?;
            return Ok(FileRun {
                document,
                output: presenter.into_buffer(),
            });
        }
    };
    let mut setup = Setup {
        chain: vec![root.join(&file)],
        deferred: Vec::new(),
    };
    let mut setup_diagnostics = String::new();
    let setup_result = run_setup(
        &spec,
        &dir,
        &fixture,
        &options.cwd,
        timeout,
        (verbose, Some(&mut setup_diagnostics)),
        &mut setup,
    );
    presenter.details(&setup_diagnostics)?;
    if let Err(error) = &setup_result {
        if cancellation::signal().is_some() {
            document.cancelled = true;
            presenter.document_cancel(&file, "Setup", error)?;
        } else {
            document.errors += 1;
            presenter.setup_fail(&file, error)?;
        }
    } else {
        defer(
            &spec,
            &dir,
            timeout,
            &file,
            indices.len(),
            &mut setup.deferred,
        );
        document.total = indices.len() + setup.deferred.len();
        for (offset, index) in indices.iter().copied().enumerate() {
            if cancellation::signal().is_some() {
                document.cancelled = true;
                break;
            }
            let case = &spec.cases[index];
            let block = presented_block(case, offset + 1, listed_total);
            let progress = presenter.block_start(&block)?;
            let mut diagnostics = String::new();
            let result = execute(
                case,
                &fixture,
                &dir,
                timeout,
                verbose,
                Some(&mut diagnostics),
                None,
            );
            let stop_file = spec.shell.is_some() && result.is_err();
            match result {
                Ok(true) => {
                    document.passed += 1;
                    presenter.block_pass(progress, &block, &diagnostics)?;
                }
                Ok(false) => document.cancelled = true,
                Err(error) if cancellation::signal().is_some() => {
                    document.cancelled = true;
                    presenter.block_cancel(progress, &file, &block, &error, &diagnostics)?;
                }
                Err(error) => {
                    document.failed += 1;
                    document.failed_headings.push(block.heading.clone());
                    presenter.block_fail(progress, &file, &block, &error, &diagnostics)?;
                }
            }
            if stop_file {
                break;
            }
        }
    }
    if setup_result.is_ok() {
        run_deferred(
            &setup.deferred,
            &fixture,
            verbose,
            &mut document,
            &mut presenter,
        )?;
    }
    if let Err(error) = fixture.close() {
        document.errors += 1;
        presenter.document_error(&file, "Fixture cleanup", &error)?;
    }
    document.cancelled |= cancellation::signal().is_some();
    document.duration = started.elapsed();
    presenter.document_finish(&document)?;
    Ok(FileRun {
        document,
        output: presenter.into_buffer(),
    })
}

fn not_run_document(file: String, total: usize) -> presentation::Document {
    presentation::Document {
        file,
        total,
        passed: 0,
        failed: 0,
        errors: 0,
        failed_headings: Vec::new(),
        cancelled: false,
        not_run: true,
        duration: Duration::ZERO,
    }
}

fn print_file_run(run: &FileRun) -> Result<()> {
    print!("{}", run.output);
    io::stdout().flush()?;
    Ok(())
}

fn run_serial_documents(
    options: &Options,
    root: &Path,
    gh_config: &Path,
    specs: Vec<(String, spec::Spec, Vec<usize>)>,
    verbose: bool,
) -> Result<bool> {
    let selected: Vec<_> = specs
        .iter()
        .map(|(file, spec, indices)| {
            (
                file.clone(),
                indices.len() + spec.cases.iter().filter(|case| case.always).count(),
            )
        })
        .collect();
    let mut presenter = presentation::Presenter::new();
    let mut documents = Vec::new();
    let mut stopped = None;
    for (index, (file, spec, indices)) in specs.into_iter().enumerate() {
        if cancellation::signal().is_some() {
            break;
        }
        let run = run_unattended_document(
            options,
            root,
            gh_config,
            (file, spec, indices),
            verbose,
            false,
        )?;
        print_file_run(&run)?;
        let failed = run.document.failed();
        documents.push(run.document);
        if options.fail_fast && failed {
            stopped = Some(index);
            break;
        }
    }
    if let Some(index) = stopped {
        documents.extend(
            selected
                .into_iter()
                .skip(index + 1)
                .map(|(file, total)| not_run_document(file, total)),
        );
    }
    presenter.summary(&documents)?;
    Ok(documents.iter().all(|document| !document.failed()))
}

fn failed_file_run(file: String, total: usize, error: anyhow::Error) -> Result<FileRun> {
    let mut presenter = presentation::Presenter::buffered();
    presenter.document_start(&file)?;
    presenter.setup_fail(&file, &error)?;
    let document = presentation::Document {
        file,
        total,
        passed: 0,
        failed: 0,
        errors: 1,
        failed_headings: Vec::new(),
        cancelled: false,
        not_run: false,
        duration: Duration::ZERO,
    };
    presenter.document_finish(&document)?;
    Ok(FileRun {
        document,
        output: presenter.into_buffer(),
    })
}

fn start_worker<T, F, S>(
    spawn: S,
    completed: mpsc::Sender<(usize, Result<T>)>,
    index: usize,
    work: F,
) -> io::Result<()>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
    S: FnOnce(Box<dyn FnOnce() + Send>) -> io::Result<()>,
{
    spawn(Box::new(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(work))
            .map_err(|payload| {
                let message = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("unknown panic");
                anyhow::anyhow!("Parallel document worker panicked: {message}")
            })
            .and_then(|result| result);
        let _ = completed.send((index, result));
    }))
}

fn spawn_worker<T, F>(
    completed: mpsc::Sender<(usize, Result<T>)>,
    index: usize,
    work: F,
) -> io::Result<()>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    start_worker(
        |task| {
            thread::Builder::new()
                .name(format!("markmason-file-{index}"))
                .spawn(task)
                .map(|_| ())
        },
        completed,
        index,
        work,
    )
}

fn run_parallel_documents(
    options: &Options,
    root: &Path,
    gh_config: &Path,
    specs: Vec<(String, spec::Spec, Vec<usize>)>,
    verbose: bool,
    workers: usize,
) -> Result<bool> {
    let selected: Vec<_> = specs
        .iter()
        .map(|(file, spec, indices)| {
            (
                file.clone(),
                indices.len() + spec.cases.iter().filter(|case| case.always).count(),
            )
        })
        .collect();
    let options = Arc::new(options.clone());
    let root = Arc::new(root.to_owned());
    let gh_config = Arc::new(gh_config.to_owned());
    let (completed, results) = mpsc::channel();
    let mut queued: VecDeque<_> = specs.into_iter().enumerate().collect();
    let mut active = 0usize;
    let mut stop_scheduling = false;
    let mut runs = BTreeMap::new();
    let mut presenter = presentation::Presenter::new();

    while !queued.is_empty() || active > 0 {
        if cancellation::signal().is_some() {
            stop_scheduling = true;
        }
        while !stop_scheduling && active < workers {
            if cancellation::signal().is_some() {
                stop_scheduling = true;
                break;
            }
            let Some((index, (_, spec, _))) = queued.front() else {
                break;
            };
            if !spec.parallel && active > 0 {
                break;
            }
            let index = *index;
            let (file, spec, indices) = queued.pop_front().unwrap().1;
            if !spec.parallel {
                let total = indices.len() + spec.cases.iter().filter(|case| case.always).count();
                let run = run_unattended_document(
                    &options,
                    &root,
                    &gh_config,
                    (file.clone(), spec, indices),
                    verbose,
                    true,
                )
                .or_else(|error| failed_file_run(file, total, error))?;
                let failed = run.document.failed();
                runs.insert(index, run);
                if options.fail_fast && failed {
                    stop_scheduling = true;
                }
                continue;
            }
            let worker_options = Arc::clone(&options);
            let root = Arc::clone(&root);
            let gh_config = Arc::clone(&gh_config);
            let completed = completed.clone();
            let failed_file = file.clone();
            let total = indices.len() + spec.cases.iter().filter(|case| case.always).count();
            let started = spawn_worker(completed, index, move || {
                run_unattended_document(
                    &worker_options,
                    &root,
                    &gh_config,
                    (file.clone(), spec, indices),
                    verbose,
                    true,
                )
                .with_context(|| format!("Parallel document {file} failed"))
            });
            match started {
                Ok(()) => active += 1,
                Err(error) => {
                    let run = failed_file_run(
                        failed_file,
                        total,
                        anyhow::Error::new(error)
                            .context("Failed to start parallel document worker"),
                    )?;
                    let failed = run.document.failed();
                    runs.insert(index, run);
                    if options.fail_fast && failed {
                        stop_scheduling = true;
                    }
                }
            }
        }
        if active == 0 {
            if stop_scheduling || queued.is_empty() {
                break;
            }
            continue;
        }
        let (index, run) = results
            .recv()
            .context("Parallel document worker stopped without reporting a result")?;
        active -= 1;
        let run = match run {
            Ok(run) => run,
            Err(error) => {
                let (file, total) = &selected[index];
                failed_file_run(file.clone(), *total, error)?
            }
        };
        if options.fail_fast && run.document.failed() {
            stop_scheduling = true;
        }
        if cancellation::signal().is_some() {
            stop_scheduling = true;
        }
        runs.insert(index, run);
    }

    while active > 0 {
        let (index, run) = results
            .recv()
            .context("Parallel document worker stopped without reporting a result")?;
        active -= 1;
        let run = match run {
            Ok(run) => run,
            Err(error) => {
                let (file, total) = &selected[index];
                failed_file_run(file.clone(), *total, error)?
            }
        };
        runs.insert(index, run);
    }

    let mut documents = Vec::new();
    for (index, (file, total)) in selected.into_iter().enumerate() {
        if let Some(run) = runs.remove(&index) {
            print_file_run(&run)?;
            documents.push(run.document);
        } else {
            documents.push(not_run_document(file, total));
        }
    }
    presenter.summary(&documents)?;
    Ok(documents.iter().all(|document| !document.failed()))
}

fn collect_inputs(
    spec: &spec::Spec,
    dir: &Path,
    chain: &mut Vec<PathBuf>,
    inputs: &mut Vec<spec::Input>,
) -> Result<()> {
    for input in &spec.inputs {
        if let Some(existing) = inputs.iter().find(|existing| {
            if cfg!(windows) {
                existing.name.eq_ignore_ascii_case(&input.name)
            } else {
                existing.name == input.name
            }
        }) {
            if existing.name != input.name {
                bail!(
                    "Conflicting input declarations {} and {}: Windows environment names are case-insensitive; use identical spelling across Markdown prerequisites",
                    existing.name,
                    input.name
                );
            }
        } else {
            inputs.push(input.clone());
        }
    }
    let Some(setup) = &spec.setup else {
        return Ok(());
    };
    let args = resolve_setup(setup, dir)?;
    if args.len() == 2
        && Path::new(&args[0])
            .file_name()
            .is_some_and(|name| name == "mm" || name == "mm.exe")
        && Path::new(&args[1])
            .extension()
            .is_some_and(|ext| ext == "md")
    {
        let path = dunce::canonicalize(dir.join(&args[1]))
            .with_context(|| format!("Cannot find Markdown setup {}", args[1]))?;
        if chain.contains(&path) {
            bail!("Circular Markdown setup: {}", path.display());
        }
        let setup = spec::parse_with_shell(&fs::read_to_string(&path)?, spec.shell)
            .with_context(|| format!("Invalid Markdown setup {}", path.display()))?;
        chain.push(path.clone());
        collect_inputs(
            &setup,
            path.parent()
                .context("Markdown setup has no parent directory")?,
            chain,
            inputs,
        )?;
        chain.pop();
    }
    Ok(())
}

fn input_environment(
    inputs: &[spec::Input],
    home_env: &str,
    file: &str,
    mut stepping: Option<&mut step::Session>,
) -> Result<Option<process::Environment>> {
    let mut env: process::Environment = env::vars_os().collect();
    let mut missing = Vec::new();
    for input in inputs {
        let reserved = [
            home_env,
            "REPO_ROOT",
            "PROJECT_NAME",
            "XDG_CONFIG_HOME",
            "GH_CONFIG_DIR",
            "NO_COLOR",
            "FORCE_COLOR",
            "GH_PROMPT_DISABLED",
            "GIT_TERMINAL_PROMPT",
            "BASH_ENV",
            "ENV",
            "SHELLOPTS",
            "BASHOPTS",
            "TERM",
        ];
        if reserved.iter().any(|name| {
            if cfg!(windows) {
                name.eq_ignore_ascii_case(&input.name)
            } else {
                *name == input.name
            }
        }) || input.name.starts_with("__markmason_")
            || (cfg!(windows) && input.name.to_ascii_lowercase().starts_with("__markmason_"))
            || (cfg!(windows)
                && ["APPDATA", "LOCALAPPDATA"]
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&input.name)))
        {
            bail!(
                "Input {} conflicts with a runner-managed environment variable",
                input.name
            );
        }
        if let Some(value) = env::var_os(&input.name).filter(|value| !value.is_empty()) {
            process::set_env(&mut env, &input.name, value);
        } else {
            missing.push(input);
        }
    }
    if stepping.is_none() && !missing.is_empty() {
        bail!(
            "Missing required inputs: {}. Set these environment variables or use --step to be prompted.",
            missing
                .iter()
                .map(|input| input.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for input in missing {
        let session = stepping
            .as_deref_mut()
            .context("Missing walkthrough input session")?;
        let Some(value) = session.prompt_input(file, input)? else {
            return Ok(None);
        };
        process::set_env(&mut env, &input.name, value);
    }
    Ok(Some(env))
}

fn run(options: Options) -> Result<bool> {
    if options.home_env.is_empty() || options.home_env.contains(['=', '\0']) {
        bail!("Invalid --home-env name");
    }
    let from_env = |flag: &Option<String>, suffix: &str| {
        flag.clone()
            .or_else(|| env::var(format!("{}{suffix}", options.env_prefix)).ok())
    };
    let include = regex(
        from_env(&options.file_pattern, "FILE_PATTERN"),
        "FILE_PATTERN",
    )?;
    let exclude = regex(
        from_env(&options.file_exclude, "FILE_EXCLUDE"),
        "FILE_EXCLUDE",
    )?;
    let include_tags = tags(from_env(&options.tags, "TAGS"));
    let exclude_tags = tags(from_env(&options.tags_exclude, "TAGS_EXCLUDE"));
    let names = options
        .name_patterns
        .iter()
        .map(|name| Regex::new(name).context("Invalid --test-name-pattern regex"))
        .collect::<Result<Vec<_>>>()?;
    let verbose = options.verbose
        || env::var("VERBOSE")
            .is_ok_and(|v| ["1", "true", "yes"].contains(&v.trim().to_lowercase().as_str()));
    let input = dunce::canonicalize(&options.spec_dir).with_context(|| {
        format!(
            "Cannot find specification path {}",
            options.spec_dir.display()
        )
    })?;
    let single_file = input.is_file();
    let mut files = Vec::new();
    let root = if single_file {
        if input.extension().is_none_or(|ext| ext != "md") {
            bail!(
                "Specification file must have a .md extension: {}",
                input.display()
            );
        }
        files.push(
            input
                .file_name()
                .context("Specification has no file name")?
                .to_string_lossy()
                .into_owned(),
        );
        input
            .parent()
            .context("Specification has no parent directory")?
            .to_owned()
    } else {
        discover(&input, &input, &mut files)?;
        input
    };
    files.sort();
    let mut specs = Vec::new();
    for file in files {
        if include.as_ref().is_some_and(|re| !re.is_match(&file))
            || exclude.as_ref().is_some_and(|re| re.is_match(&file))
        {
            continue;
        }
        let spec = spec::parse(&fs::read_to_string(root.join(&file))?)
            .with_context(|| format!("Invalid spec {file}"))?;
        if include_tags.iter().any(|tag| !spec.tags.contains(tag))
            || exclude_tags.iter().any(|tag| spec.tags.contains(tag))
        {
            continue;
        }
        let indices: Vec<_> = spec
            .cases
            .iter()
            .enumerate()
            .filter(|(_, case)| {
                !case.always
                    && (names.is_empty()
                        || names.iter().any(|re| {
                            re.is_match(&file)
                                || case.command.lines().any(|command| re.is_match(command))
                        }))
            })
            .map(|(i, _)| i)
            .collect();
        if names.is_empty() || !indices.is_empty() {
            specs.push((file, spec, indices));
        }
    }
    if specs.is_empty() {
        bail!("No markdown test specs matched in {}", root.display());
    }
    if options.step && include.is_none() && !single_file {
        let files: Vec<_> = specs.iter().map(|(file, _, _)| file.as_str()).collect();
        let Some(index) = step::pick_file(&files)? else {
            println!("Walkthrough cancelled. No commands were run.");
            return Ok(true);
        };
        specs = vec![specs.swap_remove(index)];
    }
    if options.list {
        let mut presenter = presentation::Presenter::new();
        let mut valid = true;
        for (file, spec, indices) in &specs {
            let listed_total = indices.len() + spec.cases.iter().filter(|case| case.always).count();
            if indices.is_empty() {
                presenter.list_document(file, &[])?;
                presenter.setup_fail(
                    file,
                    &anyhow::anyhow!(
                        "No test cases found. Expected a fenced markmason command block without always"
                    ),
                )?;
                valid = false;
                continue;
            }
            let mut listed = Vec::new();
            for (position, index) in indices.iter().copied().enumerate() {
                let case = &spec.cases[index];
                listed.push((
                    presented_block(case, position + 1, listed_total),
                    case.command.clone(),
                ));
            }
            for (offset, case) in spec.cases.iter().filter(|case| case.always).enumerate() {
                listed.push((
                    presented_block(case, indices.len() + offset + 1, listed_total),
                    case.command.clone(),
                ));
            }
            presenter.list_document(file, &listed)?;
        }
        return Ok(valid);
    }
    // Resolve authentication before replacing XDG_CONFIG_HOME in each fixture.
    let gh_config = Some(gh_config_dir()?);
    if !options.step {
        let gh_config = gh_config
            .as_deref()
            .context("Missing fixture configuration")?;
        return match options.parallel {
            Some(Parallelism::Auto) => {
                let workers = thread::available_parallelism()
                    .map(usize::from)
                    .unwrap_or(1)
                    .max(1);
                run_parallel_documents(&options, &root, gh_config, specs, verbose, workers)
            }
            Some(Parallelism::Jobs(workers)) => {
                run_parallel_documents(&options, &root, gh_config, specs, verbose, workers)
            }
            _ => run_serial_documents(&options, &root, gh_config, specs, verbose),
        };
    }
    let mut failed = 0;
    let mut presenter = presentation::Presenter::new();
    let mut documents = Vec::new();
    let blocks = specs
        .iter()
        .map(|(_, spec, indices)| {
            indices.len() + spec.cases.iter().filter(|case| case.always).count()
        })
        .sum();
    let mut stepping = options
        .step
        .then(|| step::Session::new(blocks))
        .transpose()?;
    let mut block_number = 0;
    let selected_documents: Vec<_> = specs
        .iter()
        .map(|(file, spec, indices)| {
            (
                file.clone(),
                indices.len() + spec.cases.iter().filter(|case| case.always).count(),
            )
        })
        .collect();
    let mut stopped_after = None;
    for (document_index, (file, spec, indices)) in specs.into_iter().enumerate() {
        if cancellation::signal().is_some() {
            break;
        }
        let first_block = block_number;
        block_number += indices.len() + spec.cases.iter().filter(|case| case.always).count();
        let listed_total = indices.len() + spec.cases.iter().filter(|case| case.always).count();
        if indices.is_empty() {
            let error = anyhow::anyhow!(
                "No test cases found. Expected a fenced markmason command block without always"
            );
            if stepping.is_none() {
                presenter.document_start(&file)?;
            }
            presenter.setup_fail(&file, &error)?;
            let document = presentation::Document {
                file,
                total: listed_total,
                passed: 0,
                failed: 0,
                errors: 1,
                failed_headings: Vec::new(),
                cancelled: false,
                not_run: false,
                duration: Duration::ZERO,
            };
            if stepping.is_none() {
                presenter.document_finish(&document)?;
            }
            documents.push(document);
            failed += 1;
            if options.fail_fast {
                stopped_after = Some(document_index);
                break;
            }
            continue;
        }
        if options.list {
            let mut listed = Vec::new();
            for (position, index) in indices.iter().copied().enumerate() {
                let case = &spec.cases[index];
                listed.push((
                    presented_block(case, position + 1, listed_total),
                    case.command.clone(),
                ));
            }
            for (offset, case) in spec.cases.iter().filter(|case| case.always).enumerate() {
                listed.push((
                    presented_block(case, indices.len() + offset + 1, listed_total),
                    case.command.clone(),
                ));
            }
            presenter.list_document(&file, &listed)?;
            continue;
        }
        let document_started = std::time::Instant::now();
        let mut document = presentation::Document {
            file: file.clone(),
            total: listed_total,
            passed: 0,
            failed: 0,
            errors: 0,
            failed_headings: Vec::new(),
            cancelled: false,
            not_run: false,
            duration: Duration::ZERO,
        };
        if stepping.is_none() {
            presenter.document_start(&file)?;
        } else if let Some(session) = stepping.as_mut() {
            session.begin_document(&file);
        }
        let timeout = spec
            .timeout
            .unwrap_or(Duration::from_secs_f64(options.timeout));
        let dir = root
            .join(&file)
            .parent()
            .context("Spec has no parent directory")?
            .to_owned();
        let fixture = (|| -> Result<Option<Fixture>> {
            let mut inputs = Vec::new();
            collect_inputs(&spec, &dir, &mut vec![root.join(&file)], &mut inputs)?;
            let Some(env) =
                input_environment(&inputs, &options.home_env, &file, stepping.as_mut())?
            else {
                return Ok(None);
            };
            cancellation::check()?;
            Fixture::new(
                &options,
                &spec,
                gh_config
                    .as_deref()
                    .context("Missing fixture configuration")?,
                env,
            )
            .map(Some)
        })();
        let fixture = match fixture {
            Ok(Some(fixture)) => fixture,
            Ok(None) => {
                document.cancelled = true;
                document.duration = document_started.elapsed();
                if stepping.is_none() {
                    presenter.document_finish(&document)?;
                }
                documents.push(document);
                break;
            }
            Err(error) => {
                document.cancelled = cancellation::signal().is_some();
                document.duration = document_started.elapsed();
                if document.cancelled {
                    presenter.document_cancel(&file, "Setup", &error)?;
                } else {
                    document.errors += 1;
                    presenter.setup_fail(&file, &error)?;
                }
                if stepping.is_none() {
                    presenter.document_finish(&document)?;
                }
                documents.push(document);
                failed += 1;
                if cancellation::signal().is_some() {
                    break;
                }
                if options.fail_fast {
                    stopped_after = Some(document_index);
                    break;
                }
                continue;
            }
        };
        let mut setup = Setup {
            chain: vec![root.join(&file)],
            deferred: Vec::new(),
        };
        let setup_result = run_setup(
            &spec,
            &dir,
            &fixture,
            &options.cwd,
            timeout,
            (verbose, None),
            &mut setup,
        );
        if let Err(error) = &setup_result {
            if cancellation::signal().is_some() {
                document.cancelled = true;
                presenter.document_cancel(&file, "Setup", error)?;
            } else {
                document.errors += 1;
                presenter.setup_fail(&file, error)?;
                failed += 1;
            }
        } else {
            defer(
                &spec,
                &dir,
                timeout,
                &file,
                indices.len(),
                &mut setup.deferred,
            );
            document.total = indices.len() + setup.deferred.len();
            for (offset, index) in indices.iter().copied().enumerate() {
                if cancellation::signal().is_some() {
                    document.cancelled = true;
                    break;
                }
                let case = &spec.cases[index];
                let block = presented_block(case, offset + 1, listed_total);
                let progress = if stepping.is_none() {
                    Some(presenter.block_start(&block)?)
                } else {
                    None
                };
                let mut diagnostics = String::new();
                let result = if let Some(session) = stepping.as_mut() {
                    let heading = presentation::full_heading(case.title.as_deref(), &case.command);
                    session
                        .begin_block(
                            &file,
                            Some(&heading),
                            first_block + offset + 1,
                            offset + 1,
                            listed_total,
                        )
                        .and_then(|()| {
                            execute(case, &fixture, &dir, timeout, false, None, Some(session))
                        })
                } else {
                    execute(
                        case,
                        &fixture,
                        &dir,
                        timeout,
                        verbose,
                        Some(&mut diagnostics),
                        None,
                    )
                };
                let stop_file = spec.shell.is_some() && result.is_err();
                match result {
                    Ok(true) => {
                        document.passed += 1;
                        if let Some(progress) = progress {
                            presenter.block_pass(progress, &block, &diagnostics)?;
                        }
                    }
                    Ok(false) => document.cancelled = true,
                    Err(error) => {
                        if cancellation::signal().is_some() {
                            document.cancelled = true;
                            if let Some(progress) = progress {
                                presenter.block_cancel(
                                    progress,
                                    &file,
                                    &block,
                                    &error,
                                    &diagnostics,
                                )?;
                            }
                        } else {
                            failed += 1;
                            document.failed += 1;
                            document.failed_headings.push(block.heading.clone());
                            if let Some(progress) = progress {
                                presenter.block_fail(
                                    progress,
                                    &file,
                                    &block,
                                    &error,
                                    &diagnostics,
                                )?;
                            }
                        }
                    }
                }
                if stop_file || stepping.as_ref().is_some_and(step::Session::stopped) {
                    break;
                }
            }
        }
        if let Some(session) = stepping.as_mut()
            && let Err(error) = session.suspend()
        {
            document.errors += 1;
            presenter.document_error(&file, "Terminal", &error)?;
            failed += 1;
        }
        if stepping.is_some() && setup_result.is_ok() {
            presenter.resume_document(&file, document_started)?;
        }
        if setup_result.is_ok() {
            let before_failed = document.failed;
            run_deferred(
                &setup.deferred,
                &fixture,
                verbose,
                &mut document,
                &mut presenter,
            )?;
            failed += document.failed - before_failed;
        }
        if let Err(error) = fixture.close() {
            document.errors += 1;
            presenter.document_error(&file, "Fixture cleanup", &error)?;
            failed += 1;
        }
        if stepping.is_some() {
            presenter.end_document_progress()?;
        }
        document.cancelled |= cancellation::signal().is_some()
            || stepping.as_ref().is_some_and(step::Session::stopped);
        document.duration = document_started.elapsed();
        if stepping.is_none() {
            presenter.document_finish(&document)?;
        }
        documents.push(document);
        if cancellation::signal().is_some() {
            break;
        }
        if stepping.as_ref().is_some_and(step::Session::stopped) {
            break;
        }
        if options.fail_fast && documents.last().is_some_and(presentation::Document::failed) {
            stopped_after = Some(document_index);
            break;
        }
    }
    if let Some(index) = stopped_after {
        documents.extend(
            selected_documents
                .into_iter()
                .skip(index + 1)
                .map(|(file, total)| presentation::Document {
                    file,
                    total,
                    passed: 0,
                    failed: 0,
                    errors: 0,
                    failed_headings: Vec::new(),
                    cancelled: false,
                    not_run: true,
                    duration: Duration::ZERO,
                }),
        );
    }
    if let Some(session) = stepping.as_mut() {
        session.finish()?;
        presenter.summary(&documents)?;
    } else if !options.list {
        presenter.summary(&documents)?;
    }
    Ok(failed == 0)
}

fn main() -> std::process::ExitCode {
    #[cfg(windows)]
    if env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--internal-exec")
    {
        process::internal_exec();
    }
    let options = Options::parse();
    let result = cancellation::install().and_then(|()| run(options));
    if let Some(signal) = cancellation::signal() {
        if let Err(error) = result {
            eprintln!("Error: {error:#}");
        }
        return std::process::ExitCode::from((128 + signal) as u8);
    }
    match result {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
