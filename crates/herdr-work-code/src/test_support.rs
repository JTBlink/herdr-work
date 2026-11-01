//! Shared Git fixtures for library and reference-UI unit tests.
use herdr_work_code::git::Git;

pub(crate) struct Repo(pub tempfile::TempDir);
impl Repo {
    pub fn new() -> Self {
        let repo = Self(tempfile::tempdir().unwrap());
        repo.run(&["init", "-qb", "main"]);
        repo
    }
    pub fn run(&self, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(self.0.path())
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.git/no-hooks",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().into()
    }
    pub fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.path().join(path), text).unwrap();
    }
    pub fn git(&self) -> Git {
        Git::discover(self.0.path()).unwrap()
    }
}
