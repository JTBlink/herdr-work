//! Windowless sidebar sidecar. GUI subsystem on Windows: it must NEVER own a
//! console — a console process launched from a herdr focus hook flashes a
//! Windows Terminal window on Windows 11 even under CREATE_NO_WINDOW, and this
//! runs on every tab/workspace focus. All herdr interaction is socket I/O.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--update-latest") => {
            if let Err(error) = herdr_work_code::updates::run() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        Some("--refresh-sidebars") => {
            if let Err(error) = herdr_work_code::updates::refresh() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }
    let mode = match std::env::args().nth(1).as_deref() {
        Some("--toggle") => {
            herdr_work_code::ensure::Mode::Toggle(herdr_work_code::state::View::Explorer)
        }
        Some("--toggle-git") => {
            herdr_work_code::ensure::Mode::Toggle(herdr_work_code::state::View::SourceControl)
        }
        Some("--show-explorer") => {
            herdr_work_code::ensure::Mode::Activate(herdr_work_code::ensure::Target::Explorer)
        }
        Some("--show-search") => {
            herdr_work_code::ensure::Mode::Activate(herdr_work_code::ensure::Target::Search)
        }
        Some("--show-git") => {
            herdr_work_code::ensure::Mode::Activate(herdr_work_code::ensure::Target::SourceControl)
        }
        Some("--quick-open") => {
            herdr_work_code::ensure::Mode::Activate(herdr_work_code::ensure::Target::QuickOpen)
        }
        _ => herdr_work_code::ensure::Mode::Ensure,
    };
    // Errors are deliberately silent: there is no console to print to, herdr
    // logs the exit, and the next focus event retries anyway.
    let _ = herdr_work_code::ensure::run(mode);
}
