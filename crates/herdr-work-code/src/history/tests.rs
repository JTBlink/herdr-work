use super::*;

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

#[test]
fn history_details_preserve_message_root_rename_binary_and_worktree_isolation() {
    let repo = Repo::new();
    assert!(
        repo.git()
            .history_graph(30, Scope::All)
            .unwrap()
            .rows
            .is_empty()
    );
    repo.write("old name.txt", "first\nsecond\n");
    repo.write("image.bin", "\0binary");
    repo.run(&["add", "."]);
    repo.run(&["commit", "-qm", "initial\n\n完整正文"]);
    let initial = CommitDetails::load(&repo.git(), "HEAD", 0).unwrap();
    assert!(initial.parents.is_empty());
    assert!(initial.message.contains("完整正文"));
    assert_eq!(initial.files.len(), 2);
    assert!(
        initial
            .files
            .iter()
            .any(|f| f.path == "image.bin" && f.added.is_none())
    );
    let renamed_path = if cfg!(windows) {
        "new name.txt"
    } else {
        "new\tname.txt"
    };
    repo.run(&["mv", "old name.txt", renamed_path]);
    repo.run(&["commit", "-qm", "rename"]);
    let renamed = CommitDetails::load(&repo.git(), "HEAD", 0).unwrap();
    assert_eq!(renamed.files[0].old_path.as_deref(), Some("old name.txt"));
    assert_eq!(renamed.files[0].path, renamed_path);
    assert_eq!(renamed.files[0].added, Some(0));
    repo.write(renamed_path, "UNCOMMITTED\n");
    let before = repo.run(&["status", "--porcelain=v1"]);
    let doc = crate::preview::commit::load(repo.0.path(), &initial.oid, 0, Some("old name.txt"));
    assert!(doc.plain_text().contains("first"));
    assert!(!doc.plain_text().contains("UNCOMMITTED"));
    assert_eq!(repo.run(&["status", "--porcelain=v1"]), before);
    let summary = crate::preview::commit::summary(repo.0.path(), renamed.clone());
    assert_eq!(
        summary.commit_file_source(renamed.lines().1).unwrap().path,
        renamed_path
    );
    assert!(CommitDetails::load(&repo.git(), "--output=pwned", 0).is_err());
    assert!(!repo.0.path().join("pwned").exists());
}

#[test]
fn history_scopes_and_merge_parents_have_consistent_files() {
    let repo = Repo::new();
    repo.run(&["commit", "--allow-empty", "-qm", "initial"]);
    repo.run(&["checkout", "-qb", "feature"]);
    repo.write("feature.txt", "feature\n");
    repo.run(&["add", "."]);
    repo.run(&["commit", "-qm", "feature"]);
    repo.run(&["checkout", "-q", "main"]);
    repo.write("main.txt", "main\n");
    repo.run(&["add", "."]);
    repo.run(&["commit", "-qm", "main"]);
    assert_eq!(
        repo.git()
            .history_graph(30, Scope::All)
            .unwrap()
            .rows
            .iter()
            .filter(|r| r.commit.is_some())
            .count(),
        3
    );
    assert_eq!(
        repo.git()
            .history_graph(30, Scope::Current)
            .unwrap()
            .rows
            .iter()
            .filter(|r| r.commit.is_some())
            .count(),
        2
    );
    repo.run(&["merge", "--no-ff", "-qm", "merge", "feature"]);
    let first = CommitDetails::load(&repo.git(), "HEAD", 0).unwrap();
    let second = CommitDetails::load(&repo.git(), "HEAD", 1).unwrap();
    assert_eq!(first.files[0].path, "feature.txt");
    assert_eq!(second.files[0].path, "main.txt");
    assert!(CommitDetails::load(&repo.git(), "HEAD", 2).is_err());
}
