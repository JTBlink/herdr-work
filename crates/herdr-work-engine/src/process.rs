//! Internal process-tree supervisor; the terminal is never inherited.
// Copyright (c) 2026 Dmitry Persiyanov. MIT; see ../LICENSE-process-runtime.
use process_wrap::std::CommandWrap;
use std::{
    io::{self, Read},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

/// How one bounded run of a tool failed.
#[derive(Debug)]
pub(crate) enum RunError {
    /// The program is not there to run.
    NotFound,
    /// It ran and exited non-zero; `stderr` carries its diagnostic.
    Failed { stderr: String },
    /// Spawning or waiting failed at the OS level.
    Io(String),
    /// The caller's stop came first: a cancel, or a deadline.
    Stopped,
}

/// Run `cmd` in its own process tree to its stdout; once `stop` says so, the whole tree ends.
pub(crate) fn run_tree(cmd: Command, stop: impl Fn() -> bool) -> Result<String, RunError> {
    let mut cmd = CommandWrap::from(cmd);
    // No stdin: the terminal belongs to the pane.
    cmd.command_mut()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    cmd.wrap(process_wrap::std::ProcessGroup::leader());
    #[cfg(windows)]
    cmd.wrap(process_wrap::std::JobObject);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(RunError::NotFound),
        Err(error) => return Err(RunError::Io(error.to_string())),
    };
    // Drained while polling, so a large answer cannot block the child.
    let stdout = read_all(child.stdout().take());
    let stderr = read_all(child.stderr().take());
    // Done once the tool exits and its pipes close; a lingering descendant is ended once.
    let mut ended = false;
    let status = loop {
        if stop() {
            // The readers are left to finish as the ended tree closes their pipes.
            let _ = child.start_kill();
            // The tool alone, and only unreaped: a job's own wait blocks on news a poll drained.
            if !ended {
                let _ = child.inner_mut().wait();
            }
            return Err(RunError::Stopped);
        }
        match child.try_wait() {
            Ok(Some(status)) if stdout.is_finished() && stderr.is_finished() => break status,
            Ok(Some(_)) if !ended => {
                ended = true;
                let _ = child.start_kill();
            }
            Ok(_) => thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                let _ = child.start_kill();
                let _ = child.inner_mut().wait();
                return Err(RunError::Io(error.to_string()));
            }
        }
    };
    let (stdout, stderr) = (
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default(),
    );
    if status.success() {
        return Ok(String::from_utf8_lossy(&stdout).into_owned());
    }
    Err(RunError::Failed {
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

/// Run a command with a deadline and process-tree cleanup.
pub fn run(command: Command, timeout: Duration) -> anyhow::Result<String> {
    let began = std::time::Instant::now();
    run_tree(command, || began.elapsed() >= timeout).map_err(|error| match error {
        RunError::NotFound => anyhow::anyhow!("Backend executable not found"),
        RunError::Failed { stderr } => anyhow::anyhow!(stderr),
        RunError::Io(message) => anyhow::anyhow!(message),
        RunError::Stopped => {
            anyhow::anyhow!("Operation timed out; verify the result before retrying")
        }
    })
}

/// Read `pipe` to its end on a thread of its own.
fn read_all(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::Instant,
    };
    /// A tool whose grandchild holds its pipes, and that waits on it when `waits`. A second
    /// grandchild writes `survived` three seconds in, unless the tree ends first.
    #[cfg(unix)]
    fn tool_with_grandchild(dir: &Path, waits: bool) -> Command {
        let script = if waits {
            r#"echo answer; sleep 60 & (sleep 3; touch "$2") & touch "$1"; wait"#
        } else {
            r#"echo answer; sleep 60 & touch "$1""#
        };
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script, "tool"])
            .arg(dir.join("ready"))
            .arg(dir.join("survived"));
        cmd
    }

    /// The same tool as a batch file, the shape `az.cmd` has.
    #[cfg(windows)]
    fn tool_with_grandchild(dir: &Path, waits: bool) -> Command {
        let survivor = dir.join("survivor.cmd");
        std::fs::write(
            &survivor,
            "@ping -n 4 127.0.0.1 >nul\r\n@echo x> \"%~1\"\r\n",
        )
        .unwrap();
        let (timed, wait) = if waits {
            (
                format!("start /b \"\" \"{}\" \"%~2\"\r\n", survivor.display()),
                "ping -n 61 127.0.0.1 >nul\r\n",
            )
        } else {
            (String::new(), "")
        };
        let script = format!(
            "@echo off\r\necho answer\r\nstart /b \"\" ping -n 61 127.0.0.1 >nul\r\n{timed}\
             echo ready> \"%~1\"\r\n{wait}"
        );
        let path = dir.join("tool.cmd");
        std::fs::write(&path, script).unwrap();
        let mut cmd = Command::new(path);
        cmd.arg(dir.join("ready")).arg(dir.join("survived"));
        cmd
    }

    /// Run `cmd` on its own thread until `stop`.
    fn spawn_run(
        cmd: Command,
        stop: impl Fn() -> bool + Send + 'static,
    ) -> mpsc::Receiver<Result<String, RunError>> {
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || done_tx.send(run_tree(cmd, stop)));
        done_rx
    }

    /// Wait until the tool's grandchild exists.
    fn await_ready(dir: &Path) {
        let started = Instant::now();
        while !dir.join("ready").exists() {
            assert!(started.elapsed() < Duration::from_secs(10), "never started");
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(300));
    }

    /// Whether the timed grandchild outlived its tree, asked past its three seconds.
    fn survived(dir: &Path, started: Instant) -> bool {
        thread::sleep(Duration::from_secs(5).saturating_sub(started.elapsed()));
        dir.join("survived").exists()
    }

    #[test]
    fn a_cancel_ends_the_tools_whole_process_tree() {
        let dir = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let done = spawn_run(tool_with_grandchild(dir.path(), true), move || {
            flag.load(Ordering::Acquire)
        });
        await_ready(dir.path());
        cancelled.store(true, Ordering::Release);
        let result = done
            .recv_timeout(Duration::from_secs(5))
            .expect("the cancel ends the run");
        assert!(matches!(result, Err(RunError::Stopped)), "{result:?}");
        assert!(
            !survived(dir.path(), started),
            "a grandchild outlived the cancel"
        );
    }

    #[test]
    fn the_deadline_ends_a_tool_that_never_exits() {
        let dir = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let deadline = started + Duration::from_secs(1);
        let cmd = tool_with_grandchild(dir.path(), true);
        let done = spawn_run(cmd, move || Instant::now() >= deadline);
        let result = done
            .recv_timeout(Duration::from_secs(6))
            .expect("the deadline ends the run");
        assert!(matches!(result, Err(RunError::Stopped)), "{result:?}");
        assert!(
            !survived(dir.path(), started),
            "a grandchild outlived the deadline"
        );
    }

    #[test]
    fn a_tool_that_exits_is_done_though_a_grandchild_holds_its_pipes() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = tool_with_grandchild(dir.path(), false);
        // Never stopped: the tool's own exit has to end the run.
        let done = spawn_run(cmd, || false);
        let result = done
            .recv_timeout(Duration::from_secs(10))
            .expect("the tool's exit ends it");
        assert_eq!(
            result.map(|out| out.trim().to_string()).ok().as_deref(),
            Some("answer")
        );
    }
}
