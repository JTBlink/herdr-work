//! Product board tabs follow Preview: focus the origin before closing our tab.
use crate::herdr::{Herdr, cli::current_socket, socket};
use crate::json::*;

pub fn owns_tab() -> bool {
    crate::store::files::herdr_work_managed()
        && std::env::var("HERDR_PLUGIN_ID").as_deref() == Ok("jtblink.herdr-work")
        && std::env::var("HERDR_PLUGIN_ENTRYPOINT_ID").as_deref() == Ok("board")
        && !super::place::own_pane().is_empty()
}

pub fn close_tab() -> Result<(), String> {
    if !owns_tab() {
        return Err("当前终端不是 HerdrWork 创建的看板标签页".into());
    }
    let socket_path = current_socket();
    let api = Herdr::new(Some(socket_path.clone()));
    let own = super::place::own_pane();
    let pane = api.pane_get(&own).ok_or("无法核实当前看板窗格")?;
    let tab = pane.s("tab_id");
    if own.is_empty() || tab.is_empty() {
        return Err("无法核实当前看板标签页".into());
    }
    let from = super::place::return_pane();
    if let Some(origin) = api
        .pane_get(&from)
        .filter(|p| p.s("workspace_id") == pane.s("workspace_id") && p.s("tab_id") != tab)
    {
        // Like Preview, force a focus transition even when the session's focus
        // record still points at the origin while this client views the board.
        let _ = socket::ask("pane.focus", &json!({"pane_id":own}), Some(&socket_path));
        socket::ask(
            "pane.focus",
            &json!({"pane_id":origin.s("pane_id")}),
            Some(&socket_path),
        )
        .map_err(|e| e.to_string())?;
    }
    socket::ask("tab.close", &json!({"tab_id":tab}), Some(&socket_path))
        .map(|_| ())
        .map_err(|e| {
            let _ = socket::ask("pane.focus", &json!({"pane_id":own}), Some(&socket_path));
            e.to_string()
        })
}
