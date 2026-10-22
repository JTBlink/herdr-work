#![cfg(feature = "sidebar-ui")]

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use herdr_work_code::{activity::InitialView, sidebar_component::Sidebar, state};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn explorer_and_git_settings_show_a_direct_project_vscode_action() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("demo");
    std::fs::create_dir(&root).unwrap();
    state::configure_product_profile(&directory.path().join("profile")).unwrap();
    state::update_state(|settings| settings.icons = Some(herdr_work_code::icons::IconTheme::Emoji));
    for initial in [InitialView::Explorer, InitialView::Git] {
        let mut sidebar = Sidebar::new(root.clone(), Some(initial));
        let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        sidebar.event(key(KeyCode::Char('s')));
        let mut terminal = Terminal::new(TestBackend::new(38, 42)).unwrap();
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "");
        assert!(text.contains("通过VSCode打开") && text.contains("仅HerdrWork"));
        assert!(
            text.find("通过VSCode打开").unwrap() < text.find("配置范围").unwrap(),
            "the project VS Code action belongs first"
        );
        assert!(
            text.find("切换目录").unwrap() < text.find("HerdrWork版本").unwrap(),
            "version belongs after the configuration rows"
        );
        if initial == InitialView::Explorer
            && let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES")
        {
            std::fs::create_dir_all(&output).unwrap();
            let cells = buffer.content.iter().map(|cell|
                serde_json::json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})
            ).collect::<Vec<_>>();
            std::fs::write(
                std::path::Path::new(&output).join("settings-vscode.json"),
                serde_json::to_vec(
                    &serde_json::json!({"width":38,"height":42,"cells":cells,"fixture":true}),
                )
                .unwrap(),
            )
            .unwrap();
        }
        assert!(sidebar.flush());
    }
}
