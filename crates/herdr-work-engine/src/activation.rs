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
const PROFILE_TOKEN: &str = "herdr-work-profile";

fn pane_metadata(pane: &str, profile: &str, instance: Option<&str>) -> Value {
    let mut tokens = json!({PROFILE_TOKEN: profile});
    if let Some(instance) = instance {
        tokens["herdr-work-instance"] = json!(instance);
        tokens["herdr-work-ready"] = json!("true");
    }
    json!({"pane_id":pane,"source":PLUGIN_ID,"tokens":tokens})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiRegistration {
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
#[derive(Serialize, Deserialize)]
struct UiBinaryIdentity {
    pane: String,
    instance: String,
    profile: String,
    binary: PathBuf,
    fingerprint: String,
}
fn identity_path(profile: &Path, pane: &str, instance: &str) -> PathBuf {
    request_path(profile, pane, instance).with_extension("identity.json")
}
fn registered_binary(profile: &Path, pane: &str, instance: &str, token: &str) -> Option<PathBuf> {
    let path = identity_path(profile, pane, instance);
    if path.metadata().ok()?.len() > 65536 {
        return None;
    }
    let identity: UiBinaryIdentity = storage::load(&path).ok()?;
    if identity.pane != pane || identity.instance != instance || identity.profile != token {
        return None;
    }
    let binary = identity.binary.canonicalize().ok()?;
    if storage::fingerprint(&std::fs::read(&binary).ok()?) != identity.fingerprint {
        return None;
    }
    Some(binary)
}
fn rpc(socket: &Path, method: &str, params: Value) -> Result<Value> {
    herdr_reviewr::herdr::herdr_work_request(socket.as_os_str(), method, params)
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
                // macOS reports argv0 as a process name even when argv[0]
                // contains the executable's absolute path. Keep an explicit
                // conflicting executable authoritative; resolve only bare names
                // through the matching absolute argv[0].
                let executable = process["argv0"].as_str();
                let path = executable
                    .and_then(|p| {
                        let path = Path::new(p);
                        if path.is_absolute() || path.components().count() > 1 {
                            path.canonicalize().ok()
                        } else if path.file_name() == binary.file_name() {
                            process["argv"][0]
                                .as_str()
                                .and_then(|p| Path::new(p).canonicalize().ok())
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        executable
                            .is_none()
                            .then(|| {
                                process["argv"][0]
                                    .as_str()
                                    .and_then(|p| Path::new(p).canonicalize().ok())
                            })
                            .flatten()
                    });
                if path.as_deref() != Some(binary) {
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
        .filter(|id| !id.is_empty())
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
            .filter(|p| p["tokens"][PROFILE_TOKEN].as_str() == Some(token.as_str()))
            .collect();
        let mut verified = vec![];
        for pane in &own {
            let Some(id) = pane["pane_id"].as_str().filter(|id| !id.is_empty()) else {
                continue;
            };
            if pane["tokens"]["herdr-work-ready"].as_str() != Some("true") {
                continue;
            }
            let Some(instance) = pane["tokens"]["herdr-work-instance"]
                .as_str()
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let info = rpc(&socket, "pane.process_info", json!({"pane_id":id}))?;
            let candidate =
                registered_binary(&profile, id, instance, &token).unwrap_or_else(|| binary.clone());
            if ui_process(&info, id, &candidate, &profile) {
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
        let target = context
            .as_ref()
            .and_then(|v| v["focused_pane_id"].as_str())
            .or_else(|| context.as_ref().and_then(|v| v["pane_id"].as_str()))
            .map(str::to_owned)
            .or_else(|| std::env::var("HERDR_PANE_ID").ok())
            .or_else(|| {
                panes
                    .iter()
                    .find(|p| p["focused"].as_bool() == Some(true))
                    .and_then(|p| p["pane_id"].as_str())
                    .map(str::to_owned)
            })
            .context("宿主没有提供要保留的原终端窗格，无法打开侧边栏")?;
        ensure!(
            panes.iter().any(|p| p["pane_id"].as_str() == Some(&target)),
            "原终端窗格已关闭"
        );
        let mut layout = rpc(&socket, "pane.layout", json!({"pane_id":target}))?;
        // Keep the caller's existing pane. Never split or adopt another plugin.
        if let Some(entries) = layout["layout"]["panes"].as_array_mut() {
            entries.retain(|p| p["pane_id"].as_str() == Some(&target));
        }
        let preferences = if profile.join("sidebar/state.json").exists() {
            storage::load::<crate::UiPreferences>(&profile.join("sidebar/state.json"))?
        } else {
            crate::UiPreferences::default()
        };
        preferences.validate()?;
        let geometry = json!({"result":layout});
        let plan = herdr_work_code::dock::column_open_plan(
            &geometry.to_string(),
            preferences.dock_right,
            preferences.sidebar_width,
        );
        let mut fields = plan.split('\t');
        ensure!(
            fields.next() == Some(target.as_str()),
            "无法核实原终端窗格的布局"
        );
        let ratio: f64 = fields.next().context("无法计算侧边栏宽度")?.parse()?;
        let swap: bool = fields.next().context("无法计算侧边栏位置")?.parse()?;
        let params = json!({"plugin_id":PLUGIN_ID,"entrypoint":"pane","placement":"split","target_pane_id":target,"direction":"right","focus":true,"cwd":root,
            "env":{"HERDR_WORK_HOME":profile,"HERDR_WORK_ROOT":root,"HERDR_WORK_INITIAL_VIEW":view.env_value()}});
        // Split placement is scoped by its target pane. The host rejects a
        // workspace selector alongside target_pane_id.
        let opened = rpc(&socket, "plugin.pane.open", params)?;
        let pane = opened_pane(&opened).context("宿主没有返回新建的自有窗格")?;
        rpc(
            &socket,
            "pane.report_metadata",
            // Only mark ownership here: the UI may already have reported its fresh
            // instance and ready flag before plugin.pane.open returned. Metadata
            // merges, so do not replace that newer readiness information.
            pane_metadata(pane, &token, None),
        )?;
        if swap {
            rpc(
                &socket,
                "pane.swap",
                json!({"source_pane_id":pane,"target_pane_id":target}),
            )?;
        }
        let amount = (ratio - 0.5).abs();
        if amount >= 0.005 {
            rpc(
                &socket,
                "pane.resize",
                json!({"pane_id":pane,"direction":if preferences.dock_right {"right"} else {"left"},"amount":amount}),
            )?;
        }
        Ok(json!({"action":"opened","pane_id":pane,"view":view}))
    }
    /// Host-provided identity is accepted only for this product's entrypoint.
    pub fn ui_registration(&self) -> Option<UiRegistration> {
        if std::env::var("HERDR_PLUGIN_ID").ok().as_deref() != Some(PLUGIN_ID) {
            return None;
        }
        let pane = std::env::var("HERDR_PANE_ID")
            .ok()
            .filter(|s| !s.is_empty())?;
        let socket = PathBuf::from(std::env::var_os("HERDR_SOCKET_PATH")?);
        Some(UiRegistration {
            socket,
            pane,
            profile: profile_token(self.root()),
            instance: nonce(),
        })
    }
    pub fn report_ui(&self, registration: &UiRegistration) -> Result<()> {
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
        let binary = std::env::current_exe()?.canonicalize()?;
        storage::save(
            &identity_path(self.root(), &registration.pane, &registration.instance),
            &UiBinaryIdentity {
                pane: registration.pane.clone(),
                instance: registration.instance.clone(),
                profile: registration.profile.clone(),
                fingerprint: storage::fingerprint(&std::fs::read(&binary)?),
                binary,
            },
        )?;
        rpc(
            &registration.socket,
            "pane.report_metadata",
            pane_metadata(
                &registration.pane,
                &registration.profile,
                Some(&registration.instance),
            ),
        )?;
        Ok(())
    }
    pub fn pending_activation(
        &self,
        registration: &UiRegistration,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_requests_use_host_token_keys_and_string_values() {
        for instance in [None, Some("ui-instance")] {
            let request = pane_metadata("own-pane", "profile-token", instance);
            assert_eq!(request["source"], PLUGIN_ID);
            let tokens = request["tokens"].as_object().unwrap();
            for (key, value) in tokens {
                assert!(!key.is_empty() && key.len() <= 32 && key.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte,b'_'|b'-')), "invalid token key: {key}");
                assert!(
                    value.is_string() || value.is_null(),
                    "invalid token value: {value}"
                );
            }
        }
    }
}
