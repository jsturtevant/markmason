mod cancellation;
mod matching;
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
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::{Builder, TempDir};

#[derive(Debug, Parser)]
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
    #[arg(long)]
    verbose: bool,
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
            Some(shell::Session::new(
                &cwd,
                &shell_env,
                spec.timeout
                    .unwrap_or(Duration::from_secs_f64(options.timeout)),
            )?)
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
    mut stepping: Option<&mut step::Session>,
) -> Result<bool> {
    let shell_mode = fixture.shell.borrow().is_some();
    let commands = if shell_mode {
        shell::commands(&case.command)?
    } else {
        case.command.lines().map(str::to_owned).collect()
    };
    let expected = case.expected.as_ref().map(|text| fixture.substitute(text));
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
            if let Some(shell) = fixture.shell.borrow_mut().as_mut() {
                shell.run(
                    &command,
                    stdin.take(),
                    shell_block.as_mut(),
                    timeout,
                    index + 1 == commands.len(),
                )
            } else if let Some(steps) = &case.pty {
                let (result, completed) = process::pty(
                    &resolve(&command, dir)?,
                    &fixture.cwd,
                    &fixture.env,
                    &steps[step_index..],
                    timeout,
                    &fixture.processes,
                    index + 1 == commands.len(),
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
                    let result = process::run(
                        &resolve(&stage, dir)?,
                        &fixture.cwd,
                        &fixture.env,
                        input,
                        timeout,
                        &fixture.processes,
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
                check_output(case, expected.as_deref(), &output, false, false)
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
    check_output(case, expected.as_deref(), &output, verbose, true).map(|()| true)
}

fn check_output(
    case: &spec::Case,
    expected: Option<&str>,
    output: &process::Output,
    verbose: bool,
    diagnostics: bool,
) -> Result<()> {
    if verbose {
        println!("ACTUAL:\n{}", output.text);
    }
    if let Some(expected) = expected {
        let result = matching::check(&output.text, expected, case.contains || case.pty.is_some());
        if verbose || (diagnostics && result.is_err()) {
            println!(
                "ACTUAL:\n{}\nDIFF (expected vs actual):\n{}",
                output.text,
                matching::diff(&output.text, expected)
            );
        }
        result?;
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

fn run_setup(
    spec: &spec::Spec,
    dir: &Path,
    fixture: &Fixture,
    setup_cwd: &Path,
    timeout: Duration,
    verbose: bool,
    chain: &mut Vec<PathBuf>,
) -> Result<()> {
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
        if chain.contains(&path) {
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
        if setup_spec.cases.is_empty() {
            bail!("No test cases found in Markdown setup {}", path.display());
        }
        let dir = path
            .parent()
            .context("Markdown setup has no parent directory")?;
        let timeout = setup_spec.timeout.unwrap_or(timeout);
        chain.push(path.clone());
        run_setup(
            &setup_spec,
            dir,
            fixture,
            &fixture.cwd,
            timeout,
            verbose,
            chain,
        )
        .with_context(|| format!("Markdown setup {} failed", path.display()))?;
        for case in &setup_spec.cases {
            cancellation::check()?;
            execute(case, fixture, dir, timeout, verbose, None).with_context(|| {
                format!("Markdown setup {} failed: {}", path.display(), case.command)
            })?;
        }
        chain.pop();
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
            println!("Setup ({setup}):\n{}", result.text);
        }
    }
    Ok(())
}

fn collect_inputs(
    spec: &spec::Spec,
    dir: &Path,
    chain: &mut Vec<PathBuf>,
    inputs: &mut Vec<spec::Input>,
) -> Result<()> {
    for input in &spec.inputs {
        if !inputs.iter().any(|existing| {
            if cfg!(windows) {
                existing.name.eq_ignore_ascii_case(&input.name)
            } else {
                existing.name == input.name
            }
        }) {
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
        if env::var_os(&input.name).is_none_or(|value| value.is_empty()) {
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
                names.is_empty()
                    || names.iter().any(|re| {
                        re.is_match(&file)
                            || case.command.lines().any(|command| re.is_match(command))
                    })
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
    // Resolve authentication before replacing XDG_CONFIG_HOME in each fixture.
    let gh_config = if options.list {
        None
    } else {
        Some(gh_config_dir()?)
    };
    let (mut passed, mut failed) = (0, 0);
    let blocks = specs.iter().map(|(_, _, indices)| indices.len()).sum();
    let mut stepping = options
        .step
        .then(|| step::Session::new(blocks))
        .transpose()?;
    let mut block_number = 0;
    for (file, spec, indices) in specs {
        cancellation::check()?;
        let first_block = block_number;
        block_number += indices.len();
        let location = if options.step {
            format!(" ({file})")
        } else {
            String::new()
        };
        if stepping.is_none() {
            println!("{file}");
        }
        if spec.cases.is_empty() {
            eprintln!(
                "  FAIL{location}: No test cases found. Expected a fenced markmason command block"
            );
            failed += 1;
            continue;
        }
        if options.list {
            for index in indices {
                println!("  {}", spec.cases[index].command.replace('\n', "\n  "));
            }
            continue;
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
            Ok(None) => break,
            Err(error) => {
                eprintln!("  FAIL setup{location}: {error:#}");
                cancellation::check()?;
                failed += 1;
                continue;
            }
        };
        let setup_result = run_setup(
            &spec,
            &dir,
            &fixture,
            &options.cwd,
            timeout,
            verbose,
            &mut vec![root.join(&file)],
        );
        if let Err(error) = setup_result {
            eprintln!("  FAIL setup{location}: {error:#}");
            failed += 1;
        } else {
            for (offset, index) in indices.into_iter().enumerate() {
                if cancellation::signal().is_some() {
                    break;
                }
                let case = &spec.cases[index];
                let label = case.command.replace('\n', "\n       ");
                let result = if let Some(session) = stepping.as_mut() {
                    session
                        .begin_block(&file, case.title.as_deref(), first_block + offset + 1)
                        .and_then(|()| execute(case, &fixture, &dir, timeout, false, Some(session)))
                } else {
                    execute(case, &fixture, &dir, timeout, verbose, None)
                };
                let stop_file = spec.shell.is_some() && result.is_err();
                match result {
                    Ok(true) => {
                        passed += 1;
                        if stepping.is_none() {
                            println!("  PASS {label}");
                        }
                    }
                    Ok(false) => {}
                    Err(error) => {
                        failed += 1;
                        if stepping.is_none() {
                            eprintln!("  FAIL {label}: {error:#}");
                        }
                    }
                }
                if stop_file || stepping.as_ref().is_some_and(step::Session::stopped) {
                    break;
                }
            }
        }
        if let Some(session) = stepping.as_mut() {
            session.suspend()?;
        }
        if let Err(error) = fixture.close() {
            eprintln!("  FAIL cleanup{location}: {error:#}");
            failed += 1;
        }
        cancellation::check()?;
        if stepping.as_ref().is_some_and(step::Session::stopped) {
            break;
        }
    }
    if let Some(session) = stepping.as_mut() {
        session.finish(passed, failed)?;
    } else if !options.list {
        println!("\npass {passed}\nfail {failed}");
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
