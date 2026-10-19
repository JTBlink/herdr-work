//! Product-owned UI preferences; never reads the standalone Sidebar's state.
use crate::{HerdrWork, storage};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UiPreferences {
    pub sidebar_width: u16,
    pub dock_right: bool,
    pub color_theme: String,
    pub icon_theme: String,
    pub show_hidden: bool,
    pub git_decorations: bool,
    pub show_git_footer: bool,
    pub show_hotkeys: bool,
    pub copy_on_select: bool,
}
impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            sidebar_width: herdr_work_code::appearance::DEFAULT_SIDEBAR_WIDTH,
            dock_right: false,
            color_theme: "vscode".into(),
            icon_theme: "emoji".into(),
            show_hidden: true,
            git_decorations: true,
            show_git_footer: true,
            show_hotkeys: true,
            copy_on_select: true,
        }
    }
}
impl UiPreferences {
    /// Apply only fields changed since `observed`, retaining later unrelated writes.
    pub fn rebase(observed: &Self, changed: &Self, latest: &Self) -> Result<Self> {
        let mut value = serde_json::to_value(latest)?;
        patch(&mut value, observed, changed)?;
        let value: Self = serde_json::from_value(value)?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.sidebar_width
                == herdr_work_code::appearance::clamp_sidebar_width(self.sidebar_width),
            "Invalid sidebar width"
        );
        ensure!(
            herdr_work_code::appearance::ColorTheme::from_state_name(&self.color_theme).is_some(),
            "Invalid color theme"
        );
        ensure!(
            self.icon_theme == "auto"
                || herdr_work_code::icons::IconTheme::from_state_name(&self.icon_theme).is_some(),
            "Invalid icon theme"
        );
        Ok(())
    }
}
impl HerdrWork {
    pub fn ui_preferences(&self) -> Result<UiPreferences> {
        let path = self.root().join("ui.json");
        let value = if path.exists() {
            storage::load(&path)?
        } else {
            UiPreferences::default()
        };
        value.validate()?;
        Ok(value)
    }
    pub fn save_ui_preferences(&self, value: &UiPreferences) -> Result<()> {
        value.validate()?;
        storage::save(&self.root().join("ui.json"), value)
    }
}

fn patch(
    value: &mut serde_json::Value,
    observed: &UiPreferences,
    changed: &UiPreferences,
) -> Result<()> {
    changed.validate()?;
    let before = serde_json::to_value(observed)?;
    let after = serde_json::to_value(changed)?;
    let target = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Invalid UI settings"))?;
    for (key, field) in after.as_object().unwrap() {
        if before.get(key) != Some(field) {
            target.insert(key.clone(), field.clone());
        }
    }
    Ok(())
}
impl HerdrWork {
    pub fn update_ui_preferences(
        &self,
        observed: &UiPreferences,
        changed: &UiPreferences,
    ) -> Result<UiPreferences> {
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.root().join("ui.lock"))?;
        fs2::FileExt::lock_exclusive(&lock)?;
        let path = self.root().join("ui.json");
        let mut raw: serde_json::Value = if path.exists() {
            storage::load(&path)?
        } else {
            serde_json::to_value(UiPreferences::default())?
        };
        patch(&mut raw, observed, changed)?;
        let saved: UiPreferences = serde_json::from_value(raw.clone())?;
        saved.validate()?;
        storage::save(&path, &raw)?;
        Ok(saved)
    }
}
