use crate::{Workdesk, storage};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportPreview {
    pub source: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub digest: String,
}
fn inventory(root: &Path) -> Result<(Vec<PathBuf>, String, u64)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for item in std::fs::read_dir(dir)? {
            let p = item?.path();
            let meta = p.symlink_metadata()?;
            ensure!(
                !meta.file_type().is_symlink(),
                "Import symlinks are not supported"
            );
            if meta.is_dir() {
                walk(&p, out)?;
            } else if meta.is_file() {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut files = vec![];
    walk(root, &mut files)?;
    files.sort();
    let mut entries = vec![];
    let mut bytes = 0;
    for p in &files {
        let content = std::fs::read(p)?;
        bytes += content.len() as u64;
        entries.push((
            p.strip_prefix(root)?.to_string_lossy().into_owned(),
            storage::fingerprint(&content),
        ));
    }
    Ok((
        files,
        storage::fingerprint(&serde_json::to_vec(&entries)?),
        bytes,
    ))
}
impl Workdesk {
    pub fn import_preview(&self, source: &Path) -> Result<ImportPreview> {
        let source = source.canonicalize()?;
        ensure!(
            !source.starts_with(self.root()) && !self.root().starts_with(&source),
            "Import source and profile overlap"
        );
        let (files, digest, bytes) = inventory(&source)?;
        Ok(ImportPreview {
            source,
            files: files.len(),
            bytes,
            digest,
        })
    }
    pub fn import_projects(&self, preview: &ImportPreview) -> Result<()> {
        ensure!(
            self.import_preview(&preview.source)?.digest == preview.digest,
            "Source changed; preview again"
        );
        let receipt = self.root().join("import.json");
        if receipt.exists() {
            let old: ImportPreview = storage::load(&receipt)?;
            ensure!(
                old.digest == preview.digest,
                "Another source was already imported"
            );
            return Ok(());
        }
        let target = self.projects_home();
        let backup = self.root().join("projects.before-import");
        ensure!(
            !backup.exists(),
            "Previous import backup exists; reconcile before importing"
        );
        let staging = tempfile::tempdir_in(self.root())?;
        let (files, _, _) = inventory(&preview.source)?;
        for p in files {
            let dest = staging.path().join(p.strip_prefix(&preview.source)?);
            std::fs::create_dir_all(dest.parent().unwrap())?;
            std::fs::copy(p, dest)?;
        }
        ensure!(
            self.import_preview(&preview.source)?.digest == preview.digest
                && inventory(staging.path())?.1 == preview.digest,
            "Source changed during import"
        );
        let staged = staging.keep();
        storage::save(
            &self.root().join("import.pending.json"),
            &json!({"phase":"import","preview":preview,"staged":staged.file_name().unwrap().to_string_lossy(),"had_target":target.exists()}),
        )?;
        self.recover_import()
    }
    /// A durable intent is written before either rename. Restart finishes only verified copies.
    pub(crate) fn recover_import(&self) -> Result<()> {
        let journal = self.root().join("import.pending.json");
        if !journal.exists() {
            return Ok(());
        }
        let entry: Value = storage::load(&journal)?;
        let preview: ImportPreview = serde_json::from_value(entry["preview"].clone())?;
        let target = self.projects_home();
        let backup = self.root().join("projects.before-import");
        let receipt = self.root().join("import.json");
        if entry["phase"] == "rollback" {
            let archived = self.root().join("projects.rolled-back");
            if !archived.exists() {
                ensure!(
                    inventory(&target)?.1 == preview.digest,
                    "Imported data changed during rollback"
                );
                std::fs::rename(&target, &archived)?;
            }
            ensure!(
                inventory(&archived)?.1 == preview.digest,
                "Rollback archive differs"
            );
            if backup.exists() {
                ensure!(
                    !target.exists(),
                    "Rollback target already exists; inspect preserved data"
                );
                std::fs::rename(backup, target)?;
            }
            if receipt.exists() {
                std::fs::rename(receipt, self.root().join("import.rolled-back.json"))?;
            }
        } else {
            ensure!(entry["phase"] == "import", "Unknown import recovery phase");
            let name = entry["staged"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing staging directory"))?;
            ensure!(
                Path::new(name)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
                    && Path::new(name).components().count() == 1,
                "Invalid staging path"
            );
            let staged = self.root().join(name);
            let installed = target.exists() && inventory(&target)?.1 == preview.digest;
            if !installed {
                ensure!(
                    staged.is_dir() && inventory(&staged)?.1 == preview.digest,
                    "Import staging missing or changed; original data preserved"
                );
                if target.exists() {
                    ensure!(
                        entry["had_target"] == true && !backup.exists(),
                        "Import target changed; inspect preserved data"
                    );
                    std::fs::rename(&target, &backup)?;
                }
                std::fs::rename(&staged, &target)?;
            } else if staged.exists() {
                std::fs::remove_dir_all(staged)?;
            }
            storage::save(&receipt, &preview)?;
        }
        std::fs::remove_file(journal)?;
        Ok(())
    }
    pub fn rollback_import(&self) -> Result<()> {
        let receipt = self.root().join("import.json");
        let old: ImportPreview = storage::load(&receipt)?;
        let target = self.projects_home();
        ensure!(
            inventory(&target)?.1 == old.digest,
            "Imported data changed; refusing to overwrite newer work"
        );
        let archived = self.root().join("projects.rolled-back");
        ensure!(!archived.exists(), "Previous rollback archive exists");
        storage::save(
            &self.root().join("import.pending.json"),
            &json!({"phase":"rollback","preview":old}),
        )?;
        self.recover_import()
    }
}
