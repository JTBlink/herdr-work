//! Source Sidebar close request, shared with the product launcher.
use crate::{ipc, launch};

pub fn request_close(panes_json: &str, pane_id: &str) -> std::io::Result<()> {
    if launch::pane_is_starting(panes_json, pane_id) {
        // No TUI event loop or in-memory draft exists yet.
        ipc::call_text("pane.close", serde_json::json!({ "pane_id": pane_id }))?;
    } else {
        // Ctrl+Q is handled before overlays/focus modes. The TUI persists any
        // Source Control draft and closes its own pane; this launcher does not
        // poll for an acknowledgement.
        ipc::call_text(
            "pane.send_input",
            serde_json::json!({ "pane_id": pane_id, "text": "", "keys": ["ctrl+q"] }),
        )?;
    }
    Ok(())
}
