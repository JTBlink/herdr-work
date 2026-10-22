#![cfg(unix)]

use herdr_work_tasks::herdr::{
    Herdr,
    cli::{Fake, Ran, set_fake},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

struct Host(RefCell<Vec<Value>>);
impl Fake for Host {
    fn run(&self, _args: &[String], _input: &str) -> Ran {
        panic!("managed board must not invoke the independent plugin CLI");
    }
    fn ask(
        &self,
        method: &str,
        params: &Value,
    ) -> Result<Value, herdr_work_tasks::herdr::cli::HerdrError> {
        assert_eq!(method, "plugin.pane.open");
        self.0.borrow_mut().push(params.clone());
        Ok(json!({"plugin_pane":{"pane":{"pane_id":"w1:p2"}}}))
    }
}

#[test]
fn managed_board_opens_the_product_entrypoint_with_its_own_profile() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("profile");
    let project = temp.path().join("demo repo");
    // This integration-test process has only one test and starts no workers.
    unsafe {
        std::env::set_var("HERDR_WORK_MANAGED", "1");
        std::env::set_var("HERDR_WORK_HOME", &home);
        std::env::set_var("HERDR_PROJECTS_HOME", home.join("projects"));
        std::env::set_var("HERDR_PROJECTS_DATA_DIR", home.join("projects/data"));
        std::env::set_var("HERDR_PROJECTS_CONFIG_DIR", home.join("projects/settings"));
        std::env::set_var("HERDR_PROJECTS_REPORTS_DIR", home.join("projects/reports"));
        std::env::set_var("HERDR_PLUGIN_ID", "herdr-projects");
        std::env::set_var(
            "HERDR_PLUGIN_CONTEXT_JSON",
            json!({"focused_pane_cwd":project}).to_string(),
        );
    }
    let host = Rc::new(Host(RefCell::new(vec![])));
    set_fake(Some(host.clone()));
    let api = Herdr::new(Some("fixture.sock".into()));
    assert_eq!(api.open_board().unwrap().as_deref(), Some("w1:p2"));
    api.open_board_popup("90%", "90%").unwrap();
    for (request, placement) in host.0.borrow().iter().zip(["tab", "popup"]) {
        assert_eq!(request["plugin_id"], "jtblink.herdr-work");
        assert_eq!(request["entrypoint"], "board");
        assert_eq!(request["placement"], placement);
        assert_eq!(request["cwd"], json!(project));
        assert_eq!(request["env"]["HERDR_WORK_HOME"], json!(home));
        assert_eq!(
            request["env"]["HERDR_PROJECTS_DATA_DIR"],
            json!(home.join("projects/data"))
        );
        assert_eq!(request["env"]["HERDR_WORK_MANAGED"], "1");
    }
    set_fake(None);
}
