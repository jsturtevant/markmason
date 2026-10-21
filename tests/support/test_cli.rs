//! Test-only app shared by the Markdown examples and Rust regression tests.
//! See README.md for its commands and how it is built.

use std::{
    env, fs,
    io::{self, Read, Write},
    path::Path,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetHandleInformation(handle: *mut std::ffi::c_void, mask: u32, flags: u32) -> i32;
}

// These children deliberately outlive their parent so the runner must reap them.
#[allow(clippy::zombie_processes)]
fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() == 1 {
        fs::write(env::var("TEST_HOME").unwrap() + "/state", "ready").unwrap();
        return;
    }
    match args[1].as_str() {
        "echo" => println!("{}", args[2..].join(" ")),
        "args" => println!("{:?}", &args[2..]),
        "lines" => {
            for line in &args[2..] {
                println!("{line}");
            }
        }
        "warn" | "fail" => {
            println!("stdout");
            eprintln!("stderr");
            if args[1] == "fail" {
                std::process::exit(2);
            }
        }
        "cat" => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s).unwrap();
            print!("{s}");
        }
        "with-env" => {
            let separator = args
                .iter()
                .position(|arg| arg == "--")
                .expect("expected -- before command");
            assert!(separator >= 2);
            let (pairs, remainder) = args[2..separator].as_chunks::<2>();
            assert!(remainder.is_empty(), "expected NAME VALUE pairs");
            let mut command = Command::new(args.get(separator + 1).expect("expected command"));
            command.args(&args[separator + 2..]);
            for [name, value] in pairs {
                command.env(name, value);
            }
            let status = command.status().expect("could not run command");
            std::process::exit(
                status
                    .code()
                    .expect("command terminated without an exit code"),
            );
        }
        "setup" | "setup-other" => {
            assert!(Path::new(&args[2]).exists());
            let value = if args[1] == "setup" { "ready" } else { "other" };
            fs::write(env::var("TEST_HOME").unwrap() + "/state", value).unwrap();
        }
        "state" => println!(
            "{}",
            fs::read_to_string(env::var("TEST_HOME").unwrap() + "/state").unwrap()
        ),
        "state-exists" => {
            let key = args.get(2).map_or("TEST_HOME", String::as_str);
            println!(
                "{}",
                Path::new(&env::var(key).unwrap()).join("state").exists()
            );
        }
        "write-state" => fs::write(
            Path::new(&env::var(&args[2]).unwrap()).join("state"),
            &args[3],
        )
        .unwrap(),
        "vars" => {
            for key in ["TEST_HOME", "XDG_CONFIG_HOME", "GH_CONFIG_DIR"] {
                println!("{}", env::var(key).unwrap());
            }
            println!("{}", env::current_dir().unwrap().display());
        }
        "cwd" => println!("{}", env::current_dir().unwrap().display()),
        "project" => println!(
            "{}",
            env::current_dir()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
        ),
        "gh-config" => println!("{}", env::var("GH_CONFIG_DIR").unwrap()),
        "where" => println!("{}", env::current_exe().unwrap().display()),
        "setup-location" => fs::write(
            env::var("TEST_HOME").unwrap() + "/state",
            env::current_exe().unwrap().display().to_string(),
        )
        .unwrap(),
        "env" => {
            for key in &args[2..] {
                println!("{}", env::var(key).unwrap());
            }
        }
        "root" => {
            assert_eq!(args[2], env::current_dir().unwrap().display().to_string());
            assert_eq!(args[3], "");
        }
        "mark" => fs::write(&args[2], "unexpected").unwrap(),
        "tree" | "drain" | "writer" => {
            let mut command = Command::new(env::current_exe().unwrap());
            command.arg("sleep");
            if args[1] == "writer" {
                #[cfg(windows)]
                {
                    use std::os::windows::io::AsRawHandle;
                    // Retain only stdin so this exercises a blocked writer, not output draining.
                    for handle in [io::stdout().as_raw_handle(), io::stderr().as_raw_handle()] {
                        assert_ne!(unsafe { SetHandleInformation(handle, 1, 0) }, 0);
                    }
                }
                command
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
            }
            let child = command.spawn().unwrap();
            let marker = format!("{}.tmp", args[2]);
            fs::write(
                &marker,
                format!(
                    "{}\n{}\n{}\n{}",
                    std::process::id(),
                    child.id(),
                    env::var("TEST_HOME").unwrap(),
                    env::var("XDG_CONFIG_HOME").unwrap()
                ),
            )
            .unwrap();
            fs::rename(marker, &args[2]).unwrap();
            if args[1] == "tree" {
                thread::sleep(Duration::from_secs(60));
            }
        }
        "sleep" => {
            println!("before timeout");
            thread::sleep(Duration::from_secs(10));
        }
        "sleep-ready" => {
            println!("before timeout");
            io::stdout().flush().unwrap();
            fs::write(&args[2], "ready").unwrap();
            thread::sleep(Duration::from_secs(10));
        }
        "pause" => thread::sleep(Duration::from_millis(350)),
        "pause-output" => {
            println!("setup-ready");
            thread::sleep(Duration::from_millis(1300));
        }
        "timeline" => {
            let now = || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
                    .to_string()
            };
            fs::write(format!("{}.start", args[2]), now()).unwrap();
            thread::sleep(Duration::from_millis(args[3].parse().unwrap()));
            fs::write(format!("{}.end", args[2]), now()).unwrap();
        }
        "background" | "background-pty" => {
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .arg("agent")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::{io::AsRawHandle, process::CommandExt};
                for handle in [
                    io::stdin().as_raw_handle(),
                    io::stdout().as_raw_handle(),
                    io::stderr().as_raw_handle(),
                ] {
                    assert_ne!(unsafe { SetHandleInformation(handle, 1, 0) }, 0);
                }
                command.creation_flags(0x8 | 0x200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
            }
            let child = command.spawn().unwrap();
            let marker = format!("{}.tmp", args[2]);
            fs::write(
                &marker,
                format!(
                    "{}\n{}\n{}\n{}",
                    std::process::id(),
                    child.id(),
                    env::var("TEST_HOME").unwrap(),
                    env::var("XDG_CONFIG_HOME").unwrap()
                ),
            )
            .unwrap();
            fs::rename(marker, &args[2]).unwrap();
            if args[1] == "background-pty" {
                print!("Ready?");
                io::stdout().flush().unwrap();
                let mut answer = String::new();
                io::stdin().read_line(&mut answer).unwrap();
            }
        }
        "agent" => {
            let home = env::var("TEST_HOME").unwrap();
            while !Path::new(&format!("{home}/request")).exists() {
                if !Path::new(&home).exists() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            fs::write(format!("{home}/ready"), "ready").unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            while Path::new(&home).exists() && std::time::Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
        }
        "await-agent" => {
            let home = env::var("TEST_HOME").unwrap();
            fs::write(format!("{home}/request"), "").unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(1);
            while !Path::new(&format!("{home}/ready")).exists()
                && std::time::Instant::now() < deadline
            {
                thread::sleep(Duration::from_millis(10));
            }
            println!("{}", fs::read_to_string(format!("{home}/ready")).unwrap());
        }
        #[cfg(unix)]
        "nonreading" => {
            assert!(
                Command::new("stty")
                    .args(["raw", "-echo"])
                    .status()
                    .unwrap()
                    .success()
            );
            fs::write(
                &args[2],
                format!(
                    "{}\n{}\n{}",
                    std::process::id(),
                    env::var("TEST_HOME").unwrap(),
                    env::var("XDG_CONFIG_HOME").unwrap()
                ),
            )
            .unwrap();
            println!("Ready?");
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(60));
        }
        "interactive" | "interactive-location" => {
            if args[1] == "interactive-location" {
                println!("{}", env::current_exe().unwrap().display());
            }
            print!("\x1b[?25l");
            io::stdout().flush().unwrap();
            io::stdout().write_all(b"\x1b[").unwrap();
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(30));
            print!("31mReady?\x1b[0m");
            io::stdout().flush().unwrap();
            let mut answer = String::new();
            io::stdin().read_line(&mut answer).unwrap();
            println!("accepted {}", answer.trim());
            print!("Ready?");
            io::stdout().flush().unwrap();
            answer.clear();
            io::stdin().read_line(&mut answer).unwrap();
            println!("done {}", answer.trim());
        }
        "premature" => println!("done"),
        "auth" => {}
        "repo" => {
            assert_eq!(&args[2], "clone");
            fs::write(Path::new(&args[4]).join("cloned"), "yes").unwrap();
        }
        "cloned" => println!("{}", fs::read_to_string("cloned").unwrap()),
        "bulk" => {
            let bytes = vec![b'x'; 256 * 1024];
            io::stdout().write_all(&bytes).unwrap();
            io::stderr().write_all(&bytes).unwrap();
        }
        _ => panic!("unknown helper arguments {args:?}"),
    }
}
