use crate::{HerdrWork, storage};
use anyhow::Result;
use serde_json::{Value, json};

impl HerdrWork {
    pub fn reconcile_operations(&self) -> Result<Value> {
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
        Ok(json!({"operations":operations,
            "note":"Check actual HEAD or Agent conversation, then explicitly resolve unknown outcomes. No operation is automatically retried."}))
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
