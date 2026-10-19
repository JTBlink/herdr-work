use crate::{HerdrWork, storage};
use anyhow::{Context, Result};
use herdr_work_core::{Execution, ExecutionId, Task, TaskId};
use serde_json::{Value, json};
use std::{path::Path, process::Command, time::Duration};

impl HerdrWork {
    pub fn tasks(&self) -> Result<Vec<Task>> {
        let raw = self.projects_command(&["herdr-work-snapshot"], None)?;
        let rows = raw["tasks"]
            .as_array()
            .context("Unsupported Projects snapshot")?;
        rows.iter()
            .map(|v| {
                let id = v["id"].as_str().context("Task without an id")?.to_owned();
                let mut task = Task {
                    id: TaskId(id.clone()),
                    title: v["title"].as_str().unwrap_or(&id).into(),
                    stage: v["stage"].as_str().unwrap_or("unknown").into(),
                    executions: vec![],
                };
                if let Some(path) = v["worktree"].as_str().filter(|s| !s.is_empty()) {
                    let root = std::path::PathBuf::from(path);
                    let link = &v["link"];
                    let identity =
                        format!("{}:{}:{}:{}", id, path, link["terminal"], link["socket"]);
                    task.executions.push(Execution {
                        id: ExecutionId(storage::key(&identity)),
                        task: TaskId(v["attemptOf"].as_str().unwrap_or(&id).into()),
                        attempt: v["attemptOf"].as_str().map(|_| id.clone()),
                        worktree_identity: Self::worktree_identity(&root).unwrap_or_default(),
                        worktree: root,
                        origin_terminal: link["terminal"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned),
                        origin_socket: link["socket"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(std::path::PathBuf::from),
                        target: None,
                    });
                }
                Ok(task)
            })
            .collect()
    }
    pub fn projects_command(&self, args: &[&str], cwd: Option<&Path>) -> Result<Value> {
        let mut cmd = Command::new(self.backend());
        cmd.args(args)
            .env("HERDR_WORK_HOME", self.root())
            .env("HERDR_PROJECTS_HOME", self.projects_home())
            .env("HERDR_PROJECTS_DATA_DIR", self.projects_home().join("data"))
            .env(
                "HERDR_PROJECTS_CONFIG_DIR",
                self.projects_home().join("settings"),
            )
            .env(
                "HERDR_PROJECTS_REPORTS_DIR",
                self.projects_home().join("reports"),
            )
            .env("HERDR_PLUGIN_ID", "jtblink.herdr-work")
            .env("HERDR_WORK_MANAGED", "1");
        if !args
            .first()
            .is_some_and(|arg| arg.starts_with("herdr-work-"))
        {
            cmd.arg("--json");
        }
        if let Some(root) = cwd {
            cmd.current_dir(root).env(
                "HERDR_PLUGIN_CONTEXT_JSON",
                serde_json::to_string(&json!({"focused_pane_cwd":root.canonicalize()?}))?,
            );
        }
            .context("Projects backend unavailable or failed")?;
        if output.is_empty() {
            return Ok(json!({"ok": true}));
        }
        serde_json::from_str(&output).context("Projects returned invalid JSON")
    }
    pub fn create_task(&self, root: &Path, title: &str) -> Result<Value> {
        anyhow::ensure!(!title.trim().is_empty(), "Task title is empty");
        self.projects_command(&["add", title], Some(root))
    }
    pub fn task_action(&self, id: &TaskId, action: &str) -> Result<Value> {
        anyhow::ensure!(
            ["delegate", "stop", "reopen"].contains(&action),
            "Unsupported task action"
        );
        let path = self.root().join("operations").join(format!(
            "{}.json",
            storage::key(&format!("{}:{action}", id.0))
        ));
        if path.exists() {
            let old: Value = storage::load(&path)?;
            anyhow::ensure!(
                old["state"] == "finished",
                "Previous task operation needs reconciliation"
            );
        }
        storage::save(&path, &json!({"task":id,"action":action,"state":"started"}))?;
        let answer = self.projects_command(&[action, &id.0], None)?;
        storage::save(
            &path,
            &json!({"task":id,"action":action,"state":"finished","result":answer}),
        )?;
        Ok(answer)
    }
    pub fn scheduler(&self, action: &str) -> Result<Value> {
        anyhow::ensure!(
            ["pause", "resume", "status"].contains(&action),
            "Unsupported scheduler action"
        );
        self.projects_command(&["herdr-work-scheduler", action], None)
    }
    pub fn reconcile_tasks(&self) -> Result<Value> {
        let mut operations = vec![];
        let dir = self.root().join("operations");
        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let path = entry?.path();
                if path.extension().is_some_and(|s| s == "json") {
                    let record: Value = storage::load(&path)?;
                    operations.push(json!({"operation":path.file_name().unwrap().to_string_lossy(),"record":record}));
                }
            }
        }
        let tasks = self.tasks();
        Ok(
            json!({"tasks":tasks.as_ref().ok(),"tasks_error":tasks.err().map(|e| e.to_string()),"operations":operations,
            "note":"Check actual task/HEAD or Agent conversation, then explicitly resolve unknown outcomes. No operation is automatically retried."}),
        )
    }
    pub fn resolve_operation(&self, name: &str, evidence: &str) -> Result<Value> {
        anyhow::ensure!(
            name.ends_with(".json") && std::path::Path::new(name).components().count() == 1,
            "Expected an operation filename"
        );
        anyhow::ensure!(
            !evidence.trim().is_empty(),
            "Reconciliation evidence is required"
        );
        let path = self.root().join("operations").join(name);
        let mut record: Value = storage::load(&path)?;
        record["resolution"] = json!({"previous":record["state"], "evidence":evidence});
        record["state"] = json!("finished");
        storage::save(&path, &record)?;
        Ok(record)
    }
}
