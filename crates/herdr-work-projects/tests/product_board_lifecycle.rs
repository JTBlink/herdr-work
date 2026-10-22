#![cfg(unix)]

use herdr_work_tasks::{
    board::{
        place,
        run::product,
        state::{Ctx, State},
        theme::Theme,
        update::{Effect, Event, update},
        view::screen,
    },
    herdr::cli::{Fake, HerdrError, Ran, set_fake},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct Host {
    opened: RefCell<Vec<Value>>,
    calls: RefCell<Vec<(String, Value)>>,
    reject_close: RefCell<bool>,
}
impl Fake for Host {
    fn run(&self, args: &[String], _: &str) -> Ran {
        let result = match args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["pane", "get", id] => {
                let (workspace, tab) = match *id {
                    "w1:p1" => ("w1", "w1:t1"),
                    "w1:p2" => ("w1", "w1:t2"),
                    "w1:p3" => ("w1", "w1:t3"),
                    "w2:p1" => ("w2", "w2:t1"),
                    "w2:p2" => ("w2", "w2:t2"),
                    _ => panic!("unexpected pane {id}"),
                };
                json!({"pane":{"pane_id":id,"workspace_id":workspace,"tab_id":tab}})
            }
            ["tab", "rename", _, _] => json!({}),
            _ => panic!("unexpected host command {args:?}"),
        };
        Ran {
            status: 0,
            stdout: json!({"result":result}).to_string(),
            stderr: String::new(),
        }
    }
    fn ask(&self, method: &str, params: &Value) -> Result<Value, HerdrError> {
        self.calls
            .borrow_mut()
            .push((method.into(), params.clone()));
        match method {
            "plugin.pane.open" => {
                self.opened.borrow_mut().push(params.clone());
                let pane = if params["workspace_id"] == "w1" {
                    "w1:p2"
                } else {
                    "w2:p2"
                };
                Ok(json!({"plugin_pane":{"pane":pane}}))
            }
            "pane.focus" => Ok(json!({})),
            "tab.close" if *self.reject_close.borrow() => Err(HerdrError {
                code: "busy".into(),
                message: "close refused".into(),
            }),
            "tab.close" => Ok(json!({})),
            _ => panic!("unexpected RPC {method}"),
        }
    }
}

fn context(workspace: &str, own: &str, from: &str) {
    // This test binary has one test, and no worker threads.
    unsafe {
        std::env::set_var("HERDR_WORKSPACE_ID", workspace);
        std::env::set_var("HERDR_PANE_ID", own);
        std::env::set_var("HERDR_WORK_BOARD_FROM_PANE", from);
        std::env::set_var(
            "HERDR_PLUGIN_CONTEXT_JSON",
            json!({"workspace_id":workspace,"focused_pane_id":from}).to_string(),
        );
    }
}

#[test]
fn board_stays_in_the_caller_workspace_and_closes_its_tab_back_to_the_last_page() {
    let temp = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("HERDR_WORK_HOME", temp.path());
        std::env::set_var("HERDR_WORK_MANAGED", "1");
        std::env::set_var("HERDR_ENV", "1");
        std::env::set_var("HERDR_PLUGIN_ID", "jtblink.herdr-work");
        std::env::set_var("HERDR_PLUGIN_ENTRYPOINT_ID", "board");
        std::env::set_var("HERDR_SOCKET_PATH", temp.path().join("host.sock"));
    }
    let host = Rc::new(Host::default());
    set_fake(Some(host.clone()));
    for workspace in ["w1", "w2"] {
        context(
            workspace,
            &format!("{workspace}:p1"),
            &format!("{workspace}:p1"),
        );
        assert_eq!(place::show(None, true), format!("{workspace}:p2"));
        context(
            workspace,
            &format!("{workspace}:p2"),
            &format!("{workspace}:p1"),
        );
        place::arrive();
    }
    assert_eq!(host.opened.borrow().len(), 2);
    // Opening again on a different page returns to this workspace's board.
    context("w1", "w1:p3", "w1:p3");
    assert_eq!(place::show(None, true), "w1:p2");
    assert_eq!(host.opened.borrow().len(), 2);
    context("w1", "w1:p2", "w1:p1");
    assert_eq!(place::return_pane(), "w1:p3");
    *host.reject_close.borrow_mut() = true;
    assert!(product::close_tab().is_err());
    assert_eq!(
        host.calls.borrow().last().unwrap(),
        &("pane.focus".into(), json!({"pane_id":"w1:p2"}))
    );
    *host.reject_close.borrow_mut() = false;
    host.calls.borrow_mut().clear();
    product::close_tab().unwrap();
    assert_eq!(
        *host.calls.borrow(),
        vec![
            ("pane.focus".into(), json!({"pane_id":"w1:p2"})),
            ("pane.focus".into(), json!({"pane_id":"w1:p3"})),
            ("tab.close".into(), json!({"tab_id":"w1:t2"})),
        ]
    );
    // The displayed control closes immediately and keeps a draft for prefs::save.
    let mut state = State::new(Ctx::default(), Default::default());
    state.input.set_text("保留草稿");
    let theme = Theme::default_dark(true);
    for (width, rows) in [(120, 32), (28, 10)] {
        let frame = screen(&mut state, &theme, width, rows);
        if width == 120
            && let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES")
        {
            std::fs::create_dir_all(&output).unwrap();
            let mut buffer = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(
                0,
                0,
                width as u16,
                rows as u16,
            ));
            herdr_work_tasks::board::run::draw::draw(&theme, &frame.lines, &mut buffer);
            let cells = buffer.content.iter().map(|cell| json!({
                "text":cell.symbol(), "fg":format!("{:?}",cell.fg), "bg":format!("{:?}",cell.bg)
            })).collect::<Vec<_>>();
            std::fs::write(
                std::path::Path::new(&output).join("projects-close.json"),
                serde_json::to_vec(
                    &json!({"width":width,"height":rows,"cells":cells,"fixture":true}),
                )
                .unwrap(),
            )
            .unwrap();
        }
        assert!(frame.lines[0].text().starts_with(" ✕ "));
        assert_eq!(
            update(&mut state, Event::Click { x: 1, y: 0 }),
            vec![Effect::Save, Effect::Close]
        );
        assert_eq!(state.value(), "保留草稿");
    }
    place::depart();
    unsafe {
        std::env::set_var("HERDR_PLUGIN_ID", "independent-plugin");
    }
    assert!(product::close_tab().is_err());
    set_fake(None);
}
