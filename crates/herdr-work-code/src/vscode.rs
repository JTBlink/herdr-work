//! External file opening, independent of host panes and plugin configuration.
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Run on a worker: a broken editor launcher must not block the TUI.
pub fn open(path: &Path, line: Option<usize>) -> io::Result<()> {
    let path = path.canonicalize()?;
    let mut missing = None;
    for program in programs() {
        let mut command = Command::new(&program);
        #[cfg(windows)]
        {
            // Run the CLI entrypoint, never the long-lived GUI process: a
            // launcher timeout must not terminate the user's editor window.
            let cli = program
                .parent()
                .unwrap_or(Path::new("."))
                .join("resources/app/out/cli.js");
            command.env("ELECTRON_RUN_AS_NODE", "1").arg(cli);
        }
        command.arg("--reuse-window");
        let mut target = OsString::from(path.as_os_str());
        if let Some(line) = line.filter(|_| path.is_file()) {
            command.arg("--goto");
            target.push(format!(":{}:1", line.max(1)));
        }
        command
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing = Some(error);
                continue;
            }
            Err(error) => return Err(error),
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(format!("VS Code 启动失败：{status}")))
                };
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "VS Code 启动超时"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "未找到 VS Code，请安装 VS Code 并启用 code 命令：{}",
            missing.map(|e| e.to_string()).unwrap_or_default()
        ),
    ))
}

fn programs() -> Vec<PathBuf> {
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let paths = vec![PathBuf::from("code")];
    #[cfg(target_os = "macos")]
    let mut paths = vec![PathBuf::from("code")];
    #[cfg(windows)]
    let mut paths = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .filter(|path| path.is_absolute())
        .map(|path| path.join("Code.exe"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    #[cfg(target_os = "macos")]
    {
        let suffix = "Visual Studio Code.app/Contents/Resources/app/bin/code";
        paths.push(Path::new("/Applications").join(suffix));
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join("Applications").join(suffix));
        }
    }
    #[cfg(windows)]
    for (variable, suffix) in [
        ("LOCALAPPDATA", "Programs/Microsoft VS Code/Code.exe"),
        ("ProgramFiles", "Microsoft VS Code/Code.exe"),
    ] {
        if let Some(base) = std::env::var_os(variable) {
            paths.push(PathBuf::from(base).join(suffix));
        }
    }
    paths
}
