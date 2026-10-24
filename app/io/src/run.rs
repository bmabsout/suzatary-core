//! The subprocesses this crate starts: fed on stdin, read whole, and killed
//! when it outlives its timeout. The one started on a pseudo-terminal
//! (`setup_token`) takes its environment from [`environment`]; the binary's two
//! operator commands run at a terminal (`script`, `replica`) keep the
//! operator's whole environment, each saying why.
//!
//! A child is given an explicit environment, never this process's whole one:
//! [`BASE`] and the names its call site passes ([`Cmd::pass`]). So a secret
//! placed in the server's environment reaches no child that did not ask for it
//! by name, least of all a check command the agent proposed or the model's CLI.
//! A cleared environment is not a sandbox: a child runs as the same user, and
//! `HOME` and `XDG_RUNTIME_DIR` lead it to the user's files, the gpg-agent socket
//! and the secret store. What it holds is only what it was not told.
//! A child leads a process group of its own, and the group is what a timeout
//! or a halt kills, so a grandchild (the `cargo` under `sh -c`) dies with it
//! and cannot hold the output pipe open.

use std::ffi::{OsStr, OsString};
use std::io::{ErrorKind, Read, Write as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use suzatary_core::excerpt::Excerpt;

/// How often a running child is asked whether it is done, late or halted.
const POLL: Duration = Duration::from_millis(20);

/// What every child is given of this process's environment: where programs
/// and the user's files are, the locale and zone, the temporary and runtime
/// directories, the TLS roots. A name ending in `*` passes every name with
/// that prefix.
pub const BASE: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LANGUAGE",
    "LC_*",
    "LOCALE_ARCHIVE",
    "TZ",
    "TZDIR",
    "TMPDIR",
    "XDG_RUNTIME_DIR",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    "XDG_CONFIG_DIRS",
    "XDG_DATA_DIRS",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NIX_SSL_CERT_FILE",
];

/// What a child that reaches the network may need beyond [`BASE`]: the proxy, and the CA
/// bundle each tool reads to trust it.
pub const NETWORK: &[&str] = &[
    "NODE_EXTRA_CA_CERTS",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "GIT_SSL_CAINFO",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "ALL_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "all_proxy",
];

/// What `nix` reads of its environment beyond [`BASE`].
pub const NIX: &[&str] = &["NIX_*"];

/// What typst reads: its package and font paths, and fontconfig's.
pub const TYPST: &[&str] = &["TYPST_*", "FONTCONFIG_*"];

/// What a child is given of this process's environment: [`BASE`] and the names `pass` lists, for
/// a child started outside [`Cmd`] (one on a pseudo-terminal) to be given the same.
pub fn environment<'a>(
    pass: impl Iterator<Item = &'a str> + Clone,
) -> impl Iterator<Item = (OsString, OsString)> {
    std::env::vars_os().filter(move |(key, _)| passes(key, pass.clone()))
}

fn passes<'a>(key: &OsStr, pass: impl Iterator<Item = &'a str>) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };
    BASE.iter()
        .copied()
        .chain(pass)
        .any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => key.starts_with(prefix),
            None => key == pattern,
        })
}

/// One program to run.
#[derive(Debug, Clone)]
pub struct Cmd {
    /// The executable, looked up on `PATH` when it has no slash.
    pub program: OsString,
    /// Its argv after the program; never a shell string.
    pub args: Vec<OsString>,
    /// Written to its stdin and closed; `None` is `/dev/null`.
    pub stdin: Option<Vec<u8>>,
    /// Names (or `PREFIX*`) passed from this process's environment beside [`BASE`].
    pub pass: Vec<String>,
    /// Variables set (`Some`) or removed (`None`) on top of what passes.
    pub env: Vec<(OsString, Option<OsString>)>,
    /// Where it runs; `None` is this process's directory.
    pub cwd: Option<PathBuf>,
    /// Past this it is killed; `None` waits for as long as it takes.
    pub timeout: Option<Duration>,
    /// Stdout bytes kept; the rest is read and dropped.
    pub max_stdout: usize,
}

/// What a finished child left.
#[derive(Debug, Clone)]
pub struct Ran {
    /// Its exit status.
    pub status: ExitStatus,
    /// Its stdout (empty from [`watch`], which hands out lines instead).
    pub stdout: Vec<u8>,
    /// Its stderr.
    pub stderr: Vec<u8>,
}

/// Why a child did not run to an exit status.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// It could not be started.
    #[error("{program} could not be run: {source}")]
    Spawn {
        /// The program, as given.
        program: String,
        /// The operating system's reason.
        source: std::io::Error,
    },
    /// It outlived its timeout and was killed.
    #[error("{program} did not finish within {after:?}")]
    TimedOut {
        /// The program, as given.
        program: String,
        /// The timeout it had.
        after: Duration,
    },
    /// The caller stopped it.
    #[error("{program} was stopped")]
    Halted {
        /// The program, as given.
        program: String,
    },
    /// Waiting on it failed.
    #[error("{program}: {source}")]
    Wait {
        /// The program, as given.
        program: String,
        /// The operating system's reason.
        source: std::io::Error,
    },
    /// It ran to an exit that was not zero ([`succeed`] only).
    #[error("{program} exited {status}: {stderr}")]
    Exited {
        /// The program, as given.
        program: String,
        /// How it exited.
        status: ExitStatus,
        /// The end of its stderr, as a log line may quote it.
        stderr: Excerpt,
    },
}

impl Cmd {
    /// `program` with no arguments, no stdin and no timeout.
    #[must_use]
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_owned(),
            args: Vec::new(),
            stdin: None,
            pass: Vec::new(),
            env: Vec::new(),
            cwd: None,
            timeout: None,
            max_stdout: usize::MAX,
        }
    }

    /// One more argument.
    #[must_use]
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    /// More arguments.
    #[must_use]
    pub fn args<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, args: I) -> Self {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    /// Pass these names (or `PREFIX*`) from this process's environment too.
    #[must_use]
    pub fn pass(mut self, names: &[&str]) -> Self {
        self.pass
            .extend(names.iter().map(|name| (*name).to_owned()));
        self
    }

    /// Set a variable.
    #[must_use]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), Some(value.as_ref().to_owned())));
        self
    }

    /// Remove a variable.
    #[must_use]
    pub fn unset(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env.push((key.as_ref().to_owned(), None));
        self
    }

    /// Run in `dir`.
    #[must_use]
    pub fn cwd(mut self, dir: impl AsRef<Path>) -> Self {
        self.cwd = Some(dir.as_ref().to_owned());
        self
    }

    /// Feed `bytes` on stdin.
    #[must_use]
    pub fn stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(bytes.into());
        self
    }

    /// Kill it after `after`.
    #[must_use]
    pub fn timeout(mut self, after: Duration) -> Self {
        self.timeout = Some(after);
        self
    }

    /// Keep at most `bytes` of stdout.
    #[must_use]
    pub fn max_stdout(mut self, bytes: usize) -> Self {
        self.max_stdout = bytes;
        self
    }

    fn name(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .env_clear()
            .envs(environment(self.pass.iter().map(String::as_str)));
        for (key, value) in &self.env {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        if let Some(dir) = &self.cwd {
            command.current_dir(dir);
        }
        command
    }

    /// Spawn, retrying `ETXTBSY`: a program another thread has just written
    /// can still be open for writing in a sibling's fork.
    fn spawn(&self) -> Result<Child, RunError> {
        let mut tries = 0;
        loop {
            match self.command().spawn() {
                Err(failed) if failed.kind() == ErrorKind::ExecutableFileBusy && tries < 50 => {
                    tries += 1;
                    std::thread::sleep(POLL);
                }
                spawned => {
                    return spawned.map_err(|source| RunError::Spawn {
                        program: self.name(),
                        source,
                    });
                }
            }
        }
    }
}

impl Ran {
    /// Whether it exited zero.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.status.success()
    }

    /// Stdout, decoded lossily.
    #[must_use]
    pub fn out(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Stderr, decoded lossily.
    #[must_use]
    pub fn err(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

/// Run `cmd` and require it to exit zero.
///
/// # Errors
/// As [`run`], plus [`RunError::Exited`] for a non-zero exit, quoting the
/// tail of its stderr.
pub fn succeed(cmd: &Cmd) -> Result<Ran, RunError> {
    zero(cmd, run(cmd)?)
}

/// `ran`, when `cmd` exited zero; [`RunError::Exited`] otherwise.
///
/// # Errors
/// [`RunError::Exited`].
pub fn zero(cmd: &Cmd, ran: Ran) -> Result<Ran, RunError> {
    if ran.ok() {
        Ok(ran)
    } else {
        Err(RunError::Exited {
            program: cmd.name(),
            status: ran.status,
            stderr: Excerpt::of(&ran.err()),
        })
    }
}

/// Run `cmd` to its exit.
///
/// # Errors
/// [`RunError`] when it cannot start, outlives its timeout, or cannot be
/// waited on. A non-zero exit is an `Ok` [`Ran`].
pub fn run(cmd: &Cmd) -> Result<Ran, RunError> {
    drive(cmd, None, &|| false)
}

/// Run `cmd`, handing each stdout line to `on_line` as it arrives and
/// killing the child as soon as `halted` answers true.
///
/// # Errors
/// As [`run`], plus [`RunError::Halted`].
pub fn watch(
    cmd: &Cmd,
    on_line: &mut dyn FnMut(&str),
    halted: &dyn Fn() -> bool,
) -> Result<Ran, RunError> {
    drive(cmd, Some(on_line), halted)
}

/// A piece of what a child said, on the one channel both of its streams share.
enum Said {
    Out(Vec<u8>),
    Err(Vec<u8>),
}

/// Read `pipe` to its end into `said`, each piece tagged by `tag`.
fn read_into(
    mut pipe: impl Read + Send + 'static,
    said: std::sync::mpsc::Sender<Said>,
    tag: fn(Vec<u8>) -> Said,
) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n @ 1..) = pipe.read(&mut buf) {
            if said.send(tag(buf[..n].to_vec())).is_err() {
                break;
            }
        }
    });
}

/// Whether `child` has exited, asked without reaping it, so its id still names its group.
fn exited(child: &Child) -> std::io::Result<bool> {
    use rustix::process::{WaitId, WaitIdOptions, waitid};
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
    Ok(waitid(
        WaitId::Pid(rustix::process::Pid::from_child(child)),
        options,
    )?
    .is_some())
}

/// Kill `child`'s group. The child leads it ([`Cmd::command`]) and is not yet reaped, so its
/// id still names the group and no other.
fn kill_group(child: &Child) {
    let _ = rustix::process::kill_process_group(
        rustix::process::Pid::from_child(child),
        rustix::process::Signal::KILL,
    );
}

/// A child is done when it has exited and both its streams have closed. When the leader exits,
/// whatever it left in its group is killed, since nothing waits for it and it would hold the
/// streams open; and the one deadline covers both streams and the exit, so a child is answered
/// within its timeout or is a [`RunError::TimedOut`].
fn drive(
    cmd: &Cmd,
    mut on_line: Option<&mut dyn FnMut(&str)>,
    halted: &dyn Fn() -> bool,
) -> Result<Ran, RunError> {
    let mut child = cmd.spawn()?;
    if let (Some(bytes), Some(mut pipe)) = (cmd.stdin.clone(), child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let (said, heard) = channel::<Said>();
    read_into(
        child.stdout.take().expect("stdout is piped"),
        said.clone(),
        Said::Out,
    );
    read_into(
        child.stderr.take().expect("stderr is piped"),
        said,
        Said::Err,
    );

    let deadline = cmd.timeout.map(|after| Instant::now() + after);
    let stop = |child: &mut Child, why: RunError| {
        kill_group(child);
        let _ = child.wait();
        why
    };
    let waiting = |source| RunError::Wait {
        program: cmd.name(),
        source,
    };

    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let (mut done, mut closed) = (false, false);
    while !(done && closed) {
        if halted() {
            return Err(stop(
                &mut child,
                RunError::Halted {
                    program: cmd.name(),
                },
            ));
        }
        if deadline.is_some_and(|at| Instant::now() >= at) {
            return Err(stop(
                &mut child,
                RunError::TimedOut {
                    program: cmd.name(),
                    after: cmd.timeout.unwrap_or_default(),
                },
            ));
        }
        if !done {
            done = match exited(&child) {
                Ok(done) => done,
                Err(source) => return Err(stop(&mut child, waiting(source))),
            };
            if done {
                kill_group(&child);
            }
        }
        if closed {
            std::thread::sleep(POLL);
            continue;
        }
        match heard.recv_timeout(POLL) {
            Ok(Said::Out(chunk)) => {
                let room = cmd.max_stdout.saturating_sub(stdout.len());
                stdout.extend_from_slice(&chunk[..chunk.len().min(room)]);
                if let Some(on_line) = on_line.as_deref_mut() {
                    while let Some(end) = stdout.iter().position(|byte| *byte == b'\n') {
                        let line: Vec<u8> = stdout.drain(..=end).collect();
                        on_line(String::from_utf8_lossy(&line).trim_end());
                    }
                }
            }
            Ok(Said::Err(chunk)) => stderr.extend_from_slice(&chunk),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => closed = true,
        }
    }
    if let Some(on_line) = on_line
        && !stdout.is_empty()
    {
        on_line(String::from_utf8_lossy(&std::mem::take(&mut stdout)).trim_end());
    }
    let status = child.wait().map_err(waiting)?;
    Ok(Ran {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_past_its_timeout_is_killed() {
        let started = Instant::now();
        let failed = run(&Cmd::new("sleep")
            .arg("5")
            .timeout(Duration::from_millis(200)))
        .expect_err("killed");
        assert!(matches!(failed, RunError::TimedOut { .. }), "{failed}");
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    /// A variable this process holds reaches a child only when its call names it.
    #[test]
    fn a_child_sees_only_what_it_is_passed() {
        let Some(held) = std::env::var_os("CARGO_MANIFEST_DIR") else {
            return;
        };
        let shown = |cmd: Cmd| {
            run(&cmd.args([
                "-c",
                "echo \"${CARGO_MANIFEST_DIR:-unset}\"; echo \"${PATH:+path}\"",
            ]))
            .expect("sh runs")
            .out()
        };
        assert_eq!(shown(Cmd::new("sh")), "unset\npath\n");
        assert_eq!(
            shown(Cmd::new("sh").pass(&["CARGO_*"])),
            format!("{}\npath\n", held.to_string_lossy())
        );
    }

    /// The hole the leader's exit left: a check sends both streams to stderr, the `sleep` it
    /// left behind held stderr open, and the answer waited for the sleep, past the timeout.
    #[test]
    fn what_a_child_leaves_behind_dies_with_it() {
        let started = Instant::now();
        let ran = run(&Cmd::new("sh")
            .args(["-c", "exec 1>&2; (sleep 30 &); echo done"])
            .timeout(Duration::from_secs(2)))
        .expect("answered within its timeout");
        assert!(ran.ok(), "{}", ran.err());
        assert_eq!(ran.err(), "done\n");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    /// The hole a timeout left: `sh` was killed and the `sleep` it started lived on, holding
    /// the output pipe.
    #[test]
    fn a_timeout_kills_the_grandchildren_too() {
        let dir = tempfile::tempdir().expect("scratch");
        let pid_file = dir.path().join("pid");
        let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
        let failed = run(&Cmd::new("sh")
            .args(["-c", &script])
            .timeout(Duration::from_millis(300)))
        .expect_err("killed");
        assert!(matches!(failed, RunError::TimedOut { .. }), "{failed}");
        let pid = std::fs::read_to_string(&pid_file).expect("the grandchild's pid");
        let alive = || {
            std::fs::read_to_string(format!("/proc/{}/stat", pid.trim())).is_ok_and(|stat| {
                !stat
                    .split(") ")
                    .nth(1)
                    .is_some_and(|rest| rest.starts_with('Z'))
            })
        };
        let started = Instant::now();
        while alive() && started.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(POLL);
        }
        assert!(
            !alive(),
            "the grandchild {} outlived the timeout",
            pid.trim()
        );
    }

    #[test]
    fn stdout_past_the_cap_is_dropped() {
        let ran =
            run(&Cmd::new("sh").args(["-c", "echo 0123456789"]).max_stdout(4)).expect("sh runs");
        assert_eq!(ran.out(), "0123");
    }

    #[test]
    fn watch_hands_out_lines_and_stops_when_halted() {
        let mut lines = Vec::new();
        let ran = watch(
            &Cmd::new("sh").args(["-c", "printf 'a\\nb\\nc'"]),
            &mut |line| lines.push(line.to_owned()),
            &|| false,
        )
        .expect("sh runs");
        assert!(ran.ok());
        assert_eq!(lines, ["a", "b", "c"]);

        let started = Instant::now();
        let halted = watch(&Cmd::new("sleep").arg("5"), &mut |_| {}, &|| {
            started.elapsed() > Duration::from_millis(100)
        })
        .expect_err("halted");
        assert!(matches!(halted, RunError::Halted { .. }));
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}
