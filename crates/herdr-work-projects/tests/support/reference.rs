//! Test-only entry for the preserved Projects source scenarios.
#[cfg(unix)]
fn main() {
    std::process::exit(herdr_work_tasks::cli::main(
        std::env::args().skip(1).collect(),
    ));
}
#[cfg(not(unix))]
fn main() {}
