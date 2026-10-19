//! Product worker entry. Isolation is intrinsic, including direct invocation.
#[cfg(unix)]
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|s| matches!(s.as_str(), "--version" | "version"))
    {
        println!("herdr-work-projects {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let home = std::env::var_os("HERDR_WORK_HOME")
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .expect("Home directory unavailable")
                .join(".herdr-work")
        });
    let home = std::path::absolute(home).expect("Invalid HerdrWork profile");
    let projects = home.join("projects");
    // Before any task library threads or callbacks start.
    unsafe {
        for (key, path) in [
            ("HERDR_WORK_HOME", home),
            ("HERDR_PROJECTS_HOME", projects.clone()),
            ("HERDR_PROJECTS_DATA_DIR", projects.join("data")),
            ("HERDR_PROJECTS_CONFIG_DIR", projects.join("settings")),
            ("HERDR_PROJECTS_REPORTS_DIR", projects.join("reports")),
        ] {
            std::env::set_var(key, path);
        }
        std::env::set_var("HERDR_WORK_MANAGED", "1");
        if std::env::var("HERDR_PLUGIN_ID").as_deref() != Ok("jtblink.herdr-work") {
            std::env::remove_var("HERDR_PLUGIN_ID");
            std::env::remove_var("HERDR_PLUGIN_CONTEXT_JSON");
        }
    }
    std::process::exit(herdr_work_tasks::cli::main(args));
}
#[cfg(not(unix))]
fn main() {
    eprintln!("HerdrWork task execution currently requires Unix");
    std::process::exit(1);
}
