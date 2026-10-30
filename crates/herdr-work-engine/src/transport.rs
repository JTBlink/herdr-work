//! Explicit host endpoints reuse the Code client's bounded socket/pipe exchange.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{ffi::OsStr, path::Path};

pub(crate) fn request(socket: &OsStr, method: &str, params: Value) -> Result<Value> {
    let raw = herdr_work_code::ipc::call_text_at(Path::new(socket), method, params)?;
    let reply: Value = serde_json::from_str(&raw).context("Host returned an unreadable reply")?;
    ensure!(reply.get("error").is_none(), "Host refused {method}");
    reply
        .get("result")
        .cloned()
        .context("Host returned no result")
}

pub(crate) enum SendFailure {
    Unknown,
    Rejected,
}

pub(crate) fn send(socket: &Path, pane: &str, text: &str) -> Result<(), SendFailure> {
    let listed = request(socket.as_os_str(), "session.snapshot", json!({}))
        .map_err(|_| SendFailure::Unknown)?;
    let agents = listed
        .get("agents")
        .unwrap_or(&listed)
        .as_array()
        .ok_or(SendFailure::Unknown)?;
    let agent = agents
        .iter()
        .find(|agent| {
            agent["pane_id"] == pane && agent["agent"].as_str().is_some_and(|name| !name.is_empty())
        })
        .ok_or(SendFailure::Rejected)?;
    let status = agent["agent_status"].as_str().ok_or(SendFailure::Unknown)?;
    if matches!(
        Some(status),
        Some("blocked" | "permission" | "needs_input" | "awaiting_approval")
    ) {
        return Err(SendFailure::Rejected);
    }
    let text = if cfg!(windows) {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_owned()
    };
    let mut body = String::new();
    for ch in text.chars() {
        body.push(ch);
        if body.ends_with("\x1b[201~") {
            body.truncate(body.len() - 6);
        }
    }
    let params = json!({"pane_id":pane, "text":format!("\x1b[200~{body}\x1b[201~")});
    if json!({"method":"pane.send_text","params":params})
        .to_string()
        .len()
        + 128
        > 256 * 1024
    {
        return Err(SendFailure::Rejected);
    }
    let raw = herdr_work_code::ipc::call_text_at(socket, "pane.send_text", params)
        .map_err(|_| SendFailure::Unknown)?;
    let reply: Value = serde_json::from_str(&raw).map_err(|_| SendFailure::Unknown)?;
    if reply.get("error").is_some() {
        return Err(SendFailure::Rejected);
    }
    if reply.get("result").is_none() {
        return Err(SendFailure::Unknown);
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
    };

    #[test]
    fn fresh_blocked_agent_never_receives_text() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("host.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let host = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["method"], "session.snapshot");
            writeln!(stream,"{}", json!({"result":{"agents":[{"pane_id":"p1","agent":"fixture","agent_status":"blocked"}]}})).unwrap();
        });
        assert!(matches!(
            send(&socket, "p1", "fixture"),
            Err(SendFailure::Rejected)
        ));
        host.join().unwrap();
    }

    #[test]
    fn ambiguous_receipt_stays_unknown_and_inner_paste_terminators_are_removed() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("host.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let host = std::thread::spawn(move || {
            for n in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                if n == 0 {
                    assert_eq!(request["method"], "session.snapshot");
                    writeln!(stream,"{}",json!({"result":{"agents":[{"pane_id":"p1","agent":"fixture","agent_status":"idle"}]}})).unwrap();
                } else {
                    assert_eq!(request["method"], "pane.send_text");
                    let text = request["params"]["text"].as_str().unwrap();
                    assert!(text.starts_with("\x1b[200~"));
                    assert_eq!(text.matches("\x1b[201~").count(), 1);
                    writeln!(stream, "{}", json!({"ambiguous":true})).unwrap();
                }
            }
        });
        assert!(matches!(
            send(&socket, "p1", "before\x1b[201~\nafter"),
            Err(SendFailure::Unknown)
        ));
        host.join().unwrap();
    }
}
