//! Optimistic settings are patched by the worker; events and drawing stay pure.
use crate::state::*;
use anyhow::Result;
use herdr_work_code::icons::IconTheme;
use herdr_work_engine::UiPreferences;

pub(crate) enum Update {
    Saved(u64, UiPreferences, Result<UiPreferences>),
    Icons(IconTheme),
    Flushed,
    Observed(UiPreferences, Result<UiPreferences>),
}
impl App {
    pub(crate) fn save_preferences(&mut self) -> Result<()> {
        if self.interaction.preference_pending.is_some()
            || self.preferences == self.interaction.preference_base
        {
            return Ok(());
        }
        self.interaction.preference_request += 1;
        let request = self.interaction.preference_request;
        let before = self.interaction.preference_base.clone();
        let after = self.preferences.clone();
        let sent = after.clone();
        self.job(move |desk| {
            let result = desk.update_ui_preferences(&before, &after);
            Ok(Data::Preferences(Update::Saved(request, after, result)))
        })?;
        self.interaction.preference_pending = Some(request);
        self.interaction.preference_sent = Some(sent);
        Ok(())
    }
    /// A FIFO barrier prevents an earlier optimistic save from winning at exit.
    pub(crate) fn flush_preferences(&mut self) -> Result<()> {
        if self.interaction.preference_pending.is_none()
            && self.preferences == self.interaction.preference_base
        {
            return Ok(());
        }
        let observed = self
            .interaction
            .preference_sent
            .clone()
            .unwrap_or_else(|| self.interaction.preference_base.clone());
        let changed = self.preferences.clone();
        let desk = self.desk.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.jobs.submit_on_shutdown(self.context.epoch, move || {
            let result = desk.update_ui_preferences(&observed, &changed).map(|_| ());
            let _ = tx.send(result);
            Ok(Data::Preferences(Update::Flushed))
        })?;
        rx.recv()
            .map_err(|_| anyhow::anyhow!("设置最终保存未收到确认"))??;
        Ok(())
    }
    pub(crate) fn tick_preferences(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        if !self.interaction.preference_poll_pending
            && now.saturating_duration_since(self.interaction.last_preference_poll)
                >= std::time::Duration::from_secs(2)
        {
            let before = self.interaction.preference_base.clone();
            self.job(move |desk| {
                Ok(Data::Preferences(Update::Observed(
                    before,
                    desk.ui_preferences(),
                )))
            })?;
            self.interaction.preference_poll_pending = true;
            self.interaction.last_preference_poll = now;
        }
        if self.preferences.icon_theme == "auto"
            && !self.interaction.icon_probe_pending
            && self.interaction.auto_icons.is_none()
        {
            self.job(|_| {
                Ok(Data::Preferences(Update::Icons(IconTheme::resolve(
                    None, None,
                ))))
            })?;
            self.interaction.icon_probe_pending = true;
        }
        Ok(())
    }
    pub(crate) fn apply_preferences(&mut self, update: Update) {
        match update {
            Update::Flushed => {}
            Update::Observed(before, result) => {
                self.interaction.preference_poll_pending = false;
                if let Ok(latest) = result {
                    let old = self.preferences.clone();
                    self.preferences = UiPreferences::rebase(&before, &self.preferences, &latest)
                        .unwrap_or_else(|_| self.preferences.clone());
                    self.interaction.preference_base = latest;
                    herdr_work_code::ui::set_color_theme(self.color_theme());
                    if old.show_hidden != self.preferences.show_hidden
                        || old.git_decorations != self.preferences.git_decorations
                    {
                        let _ = self.refresh();
                    }
                }
            }
            Update::Icons(theme) => {
                self.interaction.auto_icons = Some(theme);
                self.interaction.icon_probe_pending = false;
            }
            Update::Saved(request, sent, result) => {
                if self.interaction.preference_pending != Some(request) {
                    return;
                }
                self.interaction.preference_pending = None;
                self.interaction.preference_sent = None;
                match result {
                    Ok(latest) => {
                        let before = self.preferences.clone();
                        self.preferences = UiPreferences::rebase(&sent, &self.preferences, &latest)
                            .unwrap_or_else(|_| self.preferences.clone());
                        self.interaction.preference_base = latest;
                        herdr_work_code::ui::set_color_theme(self.color_theme());
                        self.status = "设置已保存".into();
                        if before.show_hidden != self.preferences.show_hidden
                            || before.git_decorations != self.preferences.git_decorations
                        {
                            let _ = self.refresh();
                        }
                        if let Err(error) = self.save_preferences() {
                            self.status = format!("{error:#}");
                        }
                    }
                    Err(error) => self.status = format!("设置未保存：{error:#}"),
                }
            }
        }
    }
}
