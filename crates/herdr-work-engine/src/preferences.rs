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
            show_hotkeys: true,
            copy_on_select: true,
        }
    }
}
impl UiPreferences {
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
            herdr_work_code::icons::IconTheme::from_state_name(&self.icon_theme).is_some(),
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
