use super::*;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn graph_shows_all_refs_and_aligned_commit_columns_without_changing_navigation() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Fixture 测试作者",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.git/no-hooks",
            ])
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-10-29T09:00:00+0800")
            .env("GIT_COMMITTER_DATE", "2026-10-29T09:00:00+0800")
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["init", "-qb", "main"]);
    git(&["commit", "--allow-empty", "-qm", "初始化图谱"]);
    git(&["tag", "v1.0"]);
    git(&["checkout", "-qb", "feature"]);
    git(&["commit", "--allow-empty", "-qm", "feat: 添加分支展示"]);
    git(&["checkout", "-q", "main"]);
    git(&["commit", "--allow-empty", "-qm", "fix: 展示作者与日期"]);
    git(&["merge", "--no-ff", "-qm", "merge: 合并功能分支", "feature"]);
    git(&["checkout", "-qb", "remote-only"]);
    git(&["commit", "--allow-empty", "-qm", "deadbeef 远端独有提交"]);
    let remote_oid = git(&["rev-parse", "HEAD"]);
    git(&["update-ref", "refs/remotes/origin/remote-only", &remote_oid]);
    git(&["checkout", "-q", "main"]);
    git(&["branch", "-qD", "remote-only"]);
    let before = git(&["status", "--porcelain=v1", "-b"]);
    let request = Request::History {
        root: root.into(),
        kind: Drawer::Graph,
        path: None,
    };
    let mut doc = load(&request);
    doc.context = "demo repo".into();
    let graph = doc.graph.as_ref().unwrap();
    assert_eq!(graph.commit(0).unwrap().oid, remote_oid);
    for (row, text) in graph.rows.iter().zip(graph.legacy_lines()) {
        let expected = row
            .commit
            .as_ref()
            .map(|c| crate::scm::DrawerRef::Commit(c.short.clone()))
            .unwrap_or(crate::scm::DrawerRef::None);
        assert_eq!(
            crate::scm::parse_drawer_ref(Drawer::Graph, &text),
            expected,
            "colored graph prefixes must not hide drawer commit IDs"
        );
    }
    assert_eq!(graph.rows.iter().filter(|r| r.commit.is_some()).count(), 5);
    assert!(
        graph.rows.iter().any(|r| r.commit.is_none()),
        "merge topology must survive parsing"
    );
    assert!(doc.plain_text().contains("origin/remote-only"));
    let main_row = graph
        .rows
        .iter()
        .position(|r| {
            r.commit
                .as_ref()
                .is_some_and(|c| c.refs.iter().any(|r| r.contains("HEAD -> refs/heads/main")))
        })
        .unwrap();
    let mut find = find::Find::default();
    for (width, height) in [(140, 24), (80, 14), (38, 10), (12, 5)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut body = Rect::default();
        terminal
            .draw(|frame| {
                body = draw_doc(frame, &mut doc, IconTheme::Emoji, false, None, &mut find).1;
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let screen = buffer
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            doc.rows
                .iter()
                .all(|row| row.line.width() <= usize::from(width))
        );
        assert_eq!(
            body.y, 2,
            "fixed column header must not be clickable as a commit"
        );
        assert!(history_show_request(&request, &doc, main_row).is_some());
        assert_eq!(
            history_show_request(&request, &doc, 0),
            Some(Request::Show {
                root: root.into(),
                spec: remote_oid.clone(),
                path: None
            })
        );
        if width >= 80 {
            for label in [
                "Graph",
                "提交说明",
                "日期",
                "作者",
                "Commit",
                "2026-10-29",
                "Fixture",
            ] {
                assert!(
                    screen.replace(' ', "").contains(&label.replace(' ', "")),
                    "missing {label}: {screen}"
                );
            }
        }
        if width == 140 {
            assert!(screen.contains("● main") && screen.contains("◇ v1.0"));
            assert!(screen.contains("origin/remote-only") && screen.contains("deadbeef"));
            if let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
                std::fs::create_dir_all(&output).unwrap();
                let cells = buffer.content.iter().map(|c|serde_json::json!({"text":c.symbol(),"fg":format!("{:?}",c.fg),"bg":format!("{:?}",c.bg)})).collect::<Vec<_>>();
                std::fs::write(Path::new(&output).join("git-graph-table.json"), serde_json::to_vec(&serde_json::json!({"width":width,"height":height,"cells":cells,"fixture":true})).unwrap()).unwrap();
            }
        }
    }
    // Reflow never changes the ID attached to a source row, and graph-edge
    // rows cannot navigate to a hex-looking word in the commit subject.
    let edge = doc
        .graph
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .position(|r| r.commit.is_none())
        .unwrap();
    assert!(history_show_request(&request, &doc, edge).is_none());
    assert_eq!(git(&["status", "--porcelain=v1", "-b"]), before);
    assert_eq!(
        git(&["rev-parse", "HEAD"]),
        doc.graph.as_ref().unwrap().commit(main_row).unwrap().oid
    );
}
