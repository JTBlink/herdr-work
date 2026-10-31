//! Open local web pages with the default HTTP browser, rather than their text editor association.
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
};

#[cfg(unix)]
use std::{
    io::Read,
    time::{Duration, Instant},
};

pub fn is_web_page(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["html", "htm", "xhtml"]
                .iter()
                .any(|want| ext.eq_ignore_ascii_case(want))
        })
}

#[derive(Default)]
pub struct Launcher {
    pending: Option<Receiver<(PathBuf, Result<(), String>)>>,
}
impl Launcher {
    pub fn start(&mut self, scope: &Path, path: &Path) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let (scope, path) = (scope.to_path_buf(), path.to_path_buf());
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let result = open(&path).map_err(|e| e.to_string());
            let _ = tx.send((scope, result));
        });
        true
    }
    pub fn poll(&mut self, scope: &Path) -> Option<Result<(), String>> {
        match self.pending.as_ref()?.try_recv() {
            Ok((origin, result)) => {
                self.pending = None;
                (origin == scope).then_some(result)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                Some(Err("浏览器启动任务中断，请重试".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}

/// This is a worker operation. Never invoke a shell with the page's filename.
pub fn open(path: &Path) -> io::Result<()> {
    let path = path.canonicalize()?;
    if !path.is_file() || !is_web_page(&path) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "请选择 HTML、HTM 或 XHTML 网页文件",
        ));
    }
    launch(&file_url(&path)?)
}

fn file_url(path: &Path) -> io::Result<String> {
    let text = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "网页路径不是有效 Unicode"))?;
    #[cfg(windows)]
    let text = text.replace('\\', "/");
    #[cfg(windows)]
    let text = text
        .strip_prefix("//?/UNC/")
        .map(|p| format!("//{p}"))
        .unwrap_or_else(|| text.strip_prefix("//?/").unwrap_or(&text).to_string());
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/:".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    #[cfg(windows)]
    if let Some(unc) = encoded.strip_prefix("//") {
        return Ok(format!("file://{unc}"));
    }
    Ok(format!(
        "file://{}{}",
        if encoded.starts_with('/') { "" } else { "/" },
        encoded
    ))
}

fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"HERDR_") {
            command.env_remove(key);
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

#[cfg(unix)]
fn run(mut command: Command) -> io::Result<String> {
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("浏览器启动器缺少输出通道"))?;
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout.take(16384).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait()? {
            let bytes = reader
                .join()
                .map_err(|_| io::Error::other("浏览器启动器读取中断"))??;
            return if status.success() {
                Ok(String::from_utf8_lossy(&bytes).trim().to_string())
            } else {
                Err(io::Error::other(format!("浏览器启动器退出：{status}")))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "浏览器启动器超时，请重试",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn run_launcher(mut command: Command) -> io::Result<()> {
    command.stdout(Stdio::null());
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!("浏览器启动器退出：{status}")))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "浏览器启动器超时，请重试",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(target_os = "macos")]
fn launch(url: &str) -> io::Result<()> {
    // AppKit's HTTP handler query is local and does not load this URL.
    let mut query = command("osascript");
    query.args(["-l", "JavaScript", "-e", "ObjC.import('AppKit'); var app = $.NSWorkspace.sharedWorkspace.URLForApplicationToOpenURL($.NSURL.URLWithString('https://example.invalid')); app ? ObjC.unwrap(app.path) : ''"]);
    let browser = run(query)?;
    if browser.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "未找到默认浏览器，请先设置系统默认浏览器",
        ));
    }
    let mut open = command("open");
    open.args(["-a", &browser, url]);
    run_launcher(open)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch(url: &str) -> io::Result<()> {
    let mut query = command("xdg-settings");
    query.args(["get", "default-web-browser"]);
    let desktop = run(query)?;
    if !desktop.ends_with(".desktop")
        || desktop.contains(['/', '\\'])
        || desktop.chars().any(char::is_control)
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "未找到默认浏览器，请先在桌面环境设置默认浏览器",
        ));
    }
    let mut open = command("gtk-launch");
    open.args([&desktop, url]);
    match run_launcher(open) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut roots = vec![];
            if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
                roots.push(PathBuf::from(root));
            } else if let Some(home) = std::env::var_os("HOME") {
                roots.push(PathBuf::from(home).join(".local/share"));
            }
            roots.extend(std::env::split_paths(
                &std::env::var_os("XDG_DATA_DIRS")
                    .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
            ));
            let entry = roots
                .into_iter()
                .filter(|root| root.is_absolute())
                .map(|root| root.join("applications").join(&desktop))
                .find(|entry| entry.is_file())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "未找到默认浏览器的桌面入口")
                })?;
            let mut open = command("gio");
            open.arg("launch").arg(entry).arg(url);
            run_launcher(open)
        }
        result => result,
    }
}

#[cfg(windows)]
fn launch(url: &str) -> io::Result<()> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::Shell::{
        ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE, AssocQueryStringW,
    };
    let protocol: Vec<u16> = "https\0".encode_utf16().collect();
    let mut length = 0;
    // Query the registered HTTP browser so a user's .html editor association is irrelevant.
    unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_EXECUTABLE,
            protocol.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut length,
        );
    }
    if length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "未找到系统默认浏览器",
        ));
    }
    let mut buffer = vec![0u16; length as usize];
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_EXECUTABLE,
            protocol.as_ptr(),
            std::ptr::null(),
            buffer.as_mut_ptr(),
            &mut length,
        )
    };
    if result < 0 {
        return Err(io::Error::other("无法读取系统默认浏览器"));
    }
    let end = buffer
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(buffer.len());
    let mut browser = command(OsString::from_wide(&buffer[..end]));
    browser.arg(url).stdout(Stdio::null());
    let mut child = browser.spawn()?;
    // This is the GUI browser, not a short-lived launcher. Never kill it on a timeout.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn web_page_types_and_file_urls_keep_special_filenames_as_data() {
        for name in ["index.html", "INDEX.HTM", "page.Xhtml"] {
            assert!(is_web_page(Path::new(name)));
        }
        assert!(!is_web_page(Path::new("page.html.rs")));
        assert!(!is_web_page(Path::new("image.svg")));
        let url = file_url(Path::new("/project/网页 $(touch injected)#?.html")).unwrap();
        assert_eq!(
            url,
            "file:///project/%E7%BD%91%E9%A1%B5%20%24%28touch%20injected%29%23%3F.html"
        );
    }
    #[test]
    fn missing_or_non_web_files_do_not_launch_an_application() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("readme.txt");
        std::fs::write(&file, "text").unwrap();
        assert!(open(&file).is_err());
        assert!(open(&root.path().join("missing.html")).is_err());
    }
    #[test]
    fn browser_context_action_only_appears_for_web_page_files() {
        use crate::actions::{MenuAction, MenuEntry, menu_entries_for_path};
        for (name, directory, offered) in [
            ("index.HTML", false, true),
            ("readme.md", false, false),
            ("folder.html", true, false),
        ] {
            let menu = menu_entries_for_path(Some((Path::new(name), directory)), false);
            assert_eq!(
                menu.iter()
                    .any(|entry| matches!(entry, MenuEntry::Action(MenuAction::OpenBrowser, _))),
                offered
            );
        }
    }
}
