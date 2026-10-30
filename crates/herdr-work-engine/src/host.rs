use crate::{HerdrWork, storage};
use anyhow::{Context, Result, ensure};
use herdr_work_core::{AgentTarget, Execution};
use serde_json::{Value, json};
use std::path::Path;

fn session(socket: &Path) -> Result<(Value, Vec<Value>)> {
    let value = crate::transport::request(socket.as_os_str(), "session.snapshot", json!({}))?;
    let snapshot = value.get("snapshot").unwrap_or(&value).clone();
    let agents = value
        .get("agents")
        .or_else(|| snapshot.get("agents"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok((snapshot, agents))
}

fn identity(socket: &Path, snapshot: &Value) -> Result<String> {
    if let Some(id) = snapshot["session_id"].as_str().filter(|s| !s.is_empty()) {
        return Ok(id.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = socket.metadata()?;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("Host must expose session_id for safe persisted Windows bindings")
    }
}
fn panes(snapshot: &Value) -> Vec<&Value> {
    fn collect<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
        match v {
            Value::Object(obj) => {
                if v["pane_id"].is_string() && v["terminal_id"].is_string() {
                    out.push(v);
                }
                for x in obj.values() {
                    collect(x, out);
                }
            }
            Value::Array(a) => {
                for x in a {
                    collect(x, out);
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    collect(snapshot, &mut out);
    out
}
impl HerdrWork {
    /// Let Herdr open or reuse the checkout's workspace and focus it.
    pub fn open_worktree(&self, socket: &Path, root: &Path, path: &Path) -> Result<()> {
        crate::transport::request(
            socket.as_os_str(),
            "worktree.open",
            json!({"cwd": root, "path": path, "focus": true, "trust_repository": false}),
        )?;
        Ok(())
    }
    pub fn bind(&self, execution: &Execution, pane: Option<&str>) -> Result<Execution> {
        Self::validate_execution(execution)?;
        let socket = std::env::var_os("HERDR_SOCKET_PATH")
            .map(std::path::PathBuf::from)
            .context("HERDR_SOCKET_PATH is unavailable")?;
        if let Some(expected) = &execution.origin_socket {
            ensure!(
                *expected == socket,
                "Execution belongs to another host endpoint"
            );
        }
        ensure!(
            execution.task.0 == "browse" || execution.origin_terminal.is_some() || pane.is_some(),
            "Execution has no terminal evidence; select an explicit pane to bind"
        );
        let (snapshot, agents) = session(&socket)?;
        let root = execution.worktree.canonicalize()?;
        let candidates: Vec<_> = panes(&snapshot)
            .into_iter()
            .filter(|p| {
                let Some(id) = p["pane_id"].as_str() else {
                    return false;
                };
                if execution
                    .origin_terminal
                    .as_ref()
                    .is_some_and(|expected| p["terminal_id"].as_str() != Some(expected))
                {
                    return false;
                }
                if pane.is_some_and(|requested| requested != id) {
                    return false;
                }
                agents.iter().any(|a| {
                    a["pane_id"] == id
                        && a["agent"].is_string()
                        && a["cwd"]
                            .as_str()
                            .and_then(|cwd| std::fs::canonicalize(cwd).ok())
                            .is_some_and(|cwd| cwd == root)
                })
            })
            .collect();
        ensure!(
            candidates.len() == 1,
            "Execution has zero or multiple Agent candidates; select an explicit pane"
        );
        let target = AgentTarget {
            socket: socket.clone(),
            session: identity(&socket, &snapshot)?,
            pane: candidates[0]["pane_id"].as_str().unwrap().into(),
            terminal: candidates[0]["terminal_id"].as_str().unwrap().into(),
        };
        let mut execution = execution.clone();
        execution.target = Some(target);
        storage::save(
            &self
                .root()
                .join("bindings")
                .join(format!("{}.json", storage::key(&execution.id.0))),
            &execution,
        )?;
        Ok(execution)
    }
    pub fn bound_execution(&self, execution: &Execution) -> Result<Execution> {
        let path = self
            .root()
            .join("bindings")
            .join(format!("{}.json", storage::key(&execution.id.0)));
        if !path.exists() {
            return Ok(execution.clone());
        }
        let saved: Execution = storage::load(&path)?;
        ensure!(
            saved.worktree == execution.worktree
                && saved.worktree_identity == execution.worktree_identity
                && saved.task == execution.task,
            "Stored execution binding conflicts with source task"
        );
        Ok(saved)
    }
    pub fn validate_target(&self, execution: &Execution) -> Result<AgentTarget> {
        Self::validate_execution(execution)?;
        let target = execution
            .target
            .as_ref()
            .context("Execution is not bound to an Agent")?;
        let (snapshot, agents) = session(&target.socket)?;
        ensure!(
            identity(&target.socket, &snapshot)? == target.session,
            "Host session changed; rebind execution"
        );
        let matches: Vec<_> = panes(&snapshot)
            .into_iter()
            .filter(|p| p["terminal_id"] == target.terminal)
            .collect();
        ensure!(
            matches.len() == 1,
            "Terminal disappeared or is ambiguous; rebind execution"
        );
        let pane = matches[0]["pane_id"].as_str().context("Invalid pane")?;
        let agent = agents
            .iter()
            .find(|a| a["pane_id"] == pane && a["agent"].is_string())
            .context("Bound Agent is absent")?;
        ensure!(
            Path::new(agent["cwd"].as_str().context("Agent cwd unavailable")?).canonicalize()?
                == execution.worktree.canonicalize()?,
            "Agent worktree changed"
        );
        Ok(AgentTarget {
            pane: pane.into(),
            ..target.clone()
        })
    }
    pub fn focus(&self, execution: &Execution) -> Result<()> {
        let target = self.validate_target(execution)?;
        crate::transport::request(
            target.socket.as_os_str(),
            "pane.focus",
            json!({"pane_id":target.pane}),
        )?;
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
    };

    #[test]
    fn opening_worktrees_focuses_new_or_existing_workspaces_and_reports_refusal() {
        let fixture = tempfile::tempdir().unwrap();
        let desk = HerdrWork::open(&fixture.path().join("profile"), None).unwrap();
        let socket = fixture.path().join("host.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let root = fixture.path().join("repository");
        let checkout = fixture.path().join("linked checkout");
        let expected =
            json!({"cwd": root, "path": checkout, "focus": true, "trust_repository": false});
        let server = std::thread::spawn(move || {
            for response in [
                json!({"result": {"already_open": false}}),
                json!({"result": {"already_open": true}}),
                json!({"error": {"code": "worktree_not_found", "message": "missing checkout"}}),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = String::new();
                BufReader::new(&stream).read_line(&mut request).unwrap();
                let request: Value = serde_json::from_str(&request).unwrap();
                assert_eq!(request["method"], "worktree.open");
                assert_eq!(request["params"], expected);
                writeln!(stream, "{response}").unwrap();
            }
        });
        desk.open_worktree(&socket, &root, &checkout).unwrap();
        desk.open_worktree(&socket, &root, &checkout).unwrap();
        assert!(desk.open_worktree(&socket, &root, &checkout).is_err());
        server.join().unwrap();
    }
}
