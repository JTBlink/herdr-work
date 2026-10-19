//! Own-plugin activation; the host allocates TTYs and typed requests never become input.
use crate::{HerdrWork, storage};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
pub use herdr_work_code::activity::InitialView;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub const PLUGIN_ID: &str = "jtblink.herdr-work";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkbenchRegistration {
    pub socket: PathBuf,
    pub pane: String,
    pub profile: String,
    pub instance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivationRequest {
    pub nonce: String,
    pub pane: String,
    pub profile: String,
    pub instance: String,
    pub root: PathBuf,
    pub view: InitialView,
}
fn nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
fn profile_token(profile: &Path) -> String {
    storage::fingerprint(profile.to_string_lossy().as_bytes())
}
fn request_path(profile: &Path, pane: &str, instance: &str) -> PathBuf {
    profile.join("activation").join(format!(
        "{}.json",
        storage::key(&format!("{pane}:{instance}"))
    ))
}
fn rpc(socket: &Path, method: &str, params: Value) -> Result<Value> {
}
fn ui_process(value: &Value, pane: &str, binary: &Path, profile: &Path) -> bool {
    let info = value.get("process_info").unwrap_or(value);
    if info["pane_id"].as_str() != Some(pane) {
        return false;
    }
    info["foreground_processes"]
        .as_array()
        .is_some_and(|processes| {
            processes.iter().any(|process| {
                let executable = process["argv0"]
                    .as_str()
                    .or_else(|| process["argv"][0].as_str());
                if executable
                    .and_then(|p| Path::new(p).canonicalize().ok())
                    .as_deref()
                    != Some(binary)
                {
                    return false;
                }
                let Some(argv) = process["argv"].as_array() else {
                    return false;
                };
                let args: Vec<_> = argv.iter().filter_map(Value::as_str).collect();
                let mut i = usize::from(
                    args.first()
                        .and_then(|p| Path::new(p).canonicalize().ok())
                        .as_deref()
                        == Some(binary),
                );
                while i < args.len() {
                    match args[i] {
                        "--home" => {
                            if args
                                .get(i + 1)
                                .and_then(|p| Path::new(p).canonicalize().ok())
                                .as_deref()
                                != Some(profile)
                            {
                                return false;
                            }
                            i += 2;
                        }
                        arg if arg.starts_with("--home=") => {
                            if Path::new(&arg[7..]).canonicalize().ok().as_deref() != Some(profile)
                            {
                                return false;
                            }
                            i += 1;
                        }
                        "--projects-backend" => i += 2,
                        arg if arg.starts_with("--projects-backend=") => i += 1,
                        command => return command == "ui",
                    }
                }
                false
            })
        })
}
fn opened_pane(value: &Value) -> Option<&str> {
    value["plugin_pane"]["pane"]["pane_id"]
        .as_str()
        .or_else(|| value["pane"]["pane_id"].as_str())
        .or_else(|| value["pane_id"].as_str())
}
fn context_value() -> Option<Value> {
    let text = std::env::var("HERDR_PLUGIN_CONTEXT_JSON").ok()?;
    if text.len() > 65536 {
        return None;
    }
    serde_json::from_str(&text).ok()
}
impl HerdrWork {
    /// Dispatched before opening/locking the profile. Never starts a UI process itself.
    pub fn activate(
        profile: &Path,
        explicit_root: Option<&Path>,
        view: InitialView,
    ) -> Result<Value> {
        let socket = PathBuf::from(
            std::env::var_os("HERDR_SOCKET_PATH").context("此入口需要 Herdr 宿主连接")?,
        );
        let context = context_value();
        let root = explicit_root
            .map(Path::to_path_buf)
            .or_else(|| {
                context
                    .as_ref()
                    .and_then(|v| v["focused_pane_cwd"].as_str())
                    .map(PathBuf::from)
            })
            .unwrap_or(std::env::current_dir()?)
            .canonicalize()?;
        ensure!(root.is_dir(), "请选择工作目录");
        std::fs::create_dir_all(profile)?;
        let profile = profile.canonicalize()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(profile.join("activate.lock"))?;
        lock.try_lock_exclusive()
            .context("HerdrWork 活动入口正在处理中，请稍后重试")?;
        let token = profile_token(&profile);
        let binary = std::env::current_exe()?.canonicalize()?;
        let list = rpc(&socket, "pane.list", json!({}))?;
        let panes = list["panes"].as_array().context("宿主未返回窗格列表")?;
        let own: Vec<_> = panes
            .iter()
            .filter(|p| p["tokens"][PLUGIN_ID].as_str() == Some(token.as_str()))
            .collect();
        let mut verified = vec![];
        for pane in &own {
            let Some(id) = pane["pane_id"].as_str() else {
                continue;
            };
            if pane["tokens"]["herdr-work-ready"].as_bool() != Some(true) {
                continue;
            }
            let Some(instance) = pane["tokens"]["herdr-work-instance"]
                .as_str()
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let info = rpc(&socket, "pane.process_info", json!({"pane_id":id}))?;
            if ui_process(&info, id, &binary, &profile) {
                verified.push((id.to_string(), instance.to_string()));
            }
        }
        ensure!(
            verified.len() <= 1,
            "此资料目录存在多个自有工作台，请先明确活动实例"
        );
        if let Some((pane, instance)) = verified.pop() {
            let request = ActivationRequest {
                nonce: nonce(),
                pane: pane.clone(),
                profile: token,
                instance: instance.clone(),
                root,
                view,
            };
            storage::save(&request_path(&profile, &pane, &instance), &request)?;
            rpc(&socket, "pane.focus", json!({"pane_id":pane}))?;
            return Ok(
                json!({"action":"focused","pane_id":pane,"view":view,"request":request.nonce}),
            );
        }
        ensure!(
            own.is_empty(),
            "自有窗格尚未就绪或前台身份无法核实；保留当前窗格，请稍后重试"
        );
        drop(
            crate::profile::profile_lock(&profile)
                .context("资料目录正被使用，但宿主没有可核实的自有工作台")?,
        );
        let mut params = json!({"plugin_id":PLUGIN_ID,"entrypoint":"pane","placement":"tab","focus":true,"cwd":root,
            "env":{"HERDR_WORK_HOME":profile,"HERDR_WORK_ROOT":root,"HERDR_WORK_INITIAL_VIEW":view.env_value()}});
        if let Some(workspace) = context
            .as_ref()
            .and_then(|v| v["workspace_id"].as_str())
            .or_else(|| {
                context
                    .as_ref()
                    .and_then(|v| v["focused_workspace_id"].as_str())
            })
        {
            params["workspace_id"] = json!(workspace);
        }
        let opened = rpc(&socket, "plugin.pane.open", params)?;
        let pane = opened_pane(&opened).context("宿主没有返回新建的自有窗格")?;
        rpc(
            &socket,
            "pane.report_metadata",
            // Only mark ownership here: the UI may already have reported its fresh
            // instance and ready flag before plugin.pane.open returned. Metadata
            // merges, so do not replace that newer readiness information.
            json!({"pane_id":pane,"source":PLUGIN_ID,"tokens":{PLUGIN_ID:token}}),
        )?;
        Ok(json!({"action":"opened","pane_id":pane,"view":view}))
    }
    /// Host-provided identity is accepted only for this product's entrypoint.
    pub fn workbench_registration(&self) -> Option<WorkbenchRegistration> {
        if std::env::var("HERDR_PLUGIN_ID").ok().as_deref() != Some(PLUGIN_ID) {
            return None;
        }
        let pane = std::env::var("HERDR_PANE_ID")
            .ok()
            .filter(|s| !s.is_empty())?;
        let socket = PathBuf::from(std::env::var_os("HERDR_SOCKET_PATH")?);
        Some(WorkbenchRegistration {
            socket,
            pane,
            profile: profile_token(self.root()),
            instance: nonce(),
        })
    }
    pub fn report_workbench(&self, registration: &WorkbenchRegistration) -> Result<()> {
        ensure!(
            registration.profile == profile_token(self.root()),
            "工作台资料目录身份不匹配"
        );
        let info = rpc(
            &registration.socket,
            "pane.process_info",
            json!({"pane_id":registration.pane}),
        )?;
        ensure!(
            ui_process(
                &info,
                &registration.pane,
                &std::env::current_exe()?.canonicalize()?,
                self.root()
            ),
            "工作台前台程序身份无法核实"
        );
        rpc(
            &registration.socket,
            "pane.report_metadata",
            json!({"pane_id":registration.pane,"source":PLUGIN_ID,
            "tokens":{PLUGIN_ID:registration.profile,"herdr-work-instance":registration.instance,"herdr-work-ready":true}}),
        )?;
        Ok(())
    }
    pub fn pending_activation(
        &self,
        registration: &WorkbenchRegistration,
        previous: Option<&str>,
    ) -> Result<Option<ActivationRequest>> {
        let path = request_path(self.root(), &registration.pane, &registration.instance);
        if !path.exists() {
            return Ok(None);
        }
        ensure!(path.metadata()?.len() <= 65536, "活动请求超过限制");
        let request: ActivationRequest = storage::load(&path)?;
        ensure!(
            request.profile == registration.profile
                && request.profile == profile_token(self.root())
                && request.instance == registration.instance
                && request.pane == registration.pane,
            "活动请求身份不匹配"
        );
        Ok((previous != Some(request.nonce.as_str())).then_some(request))
    }
}
