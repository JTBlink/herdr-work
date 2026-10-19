//! One terminal owner, with background jobs and upstream-backed views.
use anyhow::{Context as _, Result};
use crossterm::event::KeyCode;
use herdr_work_core::Side;

use crate::state::*;
impl App {
    pub(crate) fn switch_tab(&mut self, tab: Tab) {
        self.edit_request = None;
        if self.tab == Tab::Scm {
            self.scm_selection = self.selection;
        }
        self.tab = tab;
        self.selection = if tab == Tab::Scm {
            self.scm_selection
        } else {
            0
        };
        self.focus = if tab == Tab::Review {
            Focus::Content
        } else {
            Focus::Sidebar
        };
        if tab == Tab::Scm {
            self.scm_cursor = self
                .scm_rows()
                .iter()
                .position(|r| r.0 == Some(self.selection))
                .unwrap_or(0);
        }
    }
    pub(crate) fn key(&mut self, key: KeyCode) -> Result<bool> {
        if self.overlay.is_some() {
            if let Err(error) = self.overlay_key(key) {
                self.status = error.to_string();
            }
            return Ok(false);
        }
        if self.search_prompt() {
            match key {
                KeyCode::Esc | KeyCode::Down => {
                    self.prompt = None;
                    self.input.clear();
                }
                KeyCode::Enter => {
                    self.prompt = None;
                    self.input.clear();
                    if matches!(self.search_state, crate::loading::LoadState::Ready)
                        && !self.search_rows.is_empty()
                    {
                        self.open_search_result()?;
                    } else {
                        self.search()?;
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.prompt = match self.prompt {
                        Some(Prompt::Search(_)) if !self.search_quick => {
                            Some(Prompt::SearchInclude)
                        }
                        Some(Prompt::SearchInclude) => Some(Prompt::SearchExclude),
                        _ => None,
                    };
                    self.input = match self.prompt {
                        Some(Prompt::SearchInclude) => self.search_include.clone(),
                        Some(Prompt::SearchExclude) => self.search_exclude.clone(),
                        _ => String::new(),
                    };
                }
                KeyCode::Backspace => {
                    self.input.pop();
                    self.search_input_changed()?;
                }
                KeyCode::Char(c) => {
                    self.input.push(c);
                    self.search_input_changed()?;
                }
                _ => {}
            }
            return Ok(false);
        }
        if self.prompt.is_some() {
            match key {
                KeyCode::Esc => {
                    self.prompt = None;
                    self.input.clear();
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Char(c) => self.input.push(c),
                KeyCode::Enter => {
                    if let Err(e) = self.submit_prompt() {
                        self.status = e.to_string();
                    }
                }
                _ => {}
            }
            return Ok(false);
        }
        let result = self.action(key);
        if let Err(e) = result {
            self.status = e.to_string();
        }
        Ok(key == KeyCode::Char('q'))
    }
    pub(crate) fn action(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char(',') => self.open_settings(),
            KeyCode::Char('m') => self.open_menu(true),
            KeyCode::Char('e') if matches!(self.tab, Tab::Files | Tab::Search) => {
                self.start_editor()?
            }
            KeyCode::Char('/') => self.query_prompt(false),
            KeyCode::Char('5') => self.switch_tab(Tab::Search),
            KeyCode::Char('c' | 'w' | '*')
                if self.tab == Tab::Search && self.focus == Focus::Sidebar =>
            {
                match key {
                    KeyCode::Char('c') => {
                        self.search_options.match_case = !self.search_options.match_case
                    }
                    KeyCode::Char('w') => {
                        self.search_options.whole_word = !self.search_options.whole_word
                    }
                    _ => self.search_options.regex = !self.search_options.regex,
                }
                self.search()?;
            }
            KeyCode::Char('i' | 'o') if self.tab == Tab::Search => {
                let include = key == KeyCode::Char('i');
                self.prompt = Some(if include {
                    Prompt::SearchInclude
                } else {
                    Prompt::SearchExclude
                });
                self.input = if include {
                    self.search_include.clone()
                } else {
                    self.search_exclude.clone()
                };
            }
            KeyCode::Char('1') => self.switch_tab(Tab::Tasks),
            KeyCode::Char('2') => self.switch_tab(Tab::Files),
            KeyCode::Char('3') => self.switch_tab(Tab::Scm),
            KeyCode::Char('4') => self.switch_tab(Tab::Review),
            KeyCode::Down | KeyCode::Up | KeyCode::Char('j' | 'k') if self.tab == Tab::Scm => {
                let down = matches!(key, KeyCode::Down | KeyCode::Char('j'));
                if self.focus == Focus::Content && self.active_drawer.is_some() {
                    self.drawer_scroll = if down {
                        self.drawer_scroll.saturating_add(1)
                    } else {
                        self.drawer_scroll.saturating_sub(1)
                    };
                } else {
                    let rows = self.scm_rows();
                    self.scm_cursor = if down {
                        (self.scm_cursor + 1).min(rows.len().saturating_sub(1))
                    } else {
                        self.scm_cursor.saturating_sub(1)
                    };
                    if let Some(index) = rows.get(self.scm_cursor).and_then(|r| r.0) {
                        self.selection = index;
                        self.scm_selection = index;
                    }
                }
            }
            KeyCode::Tab | KeyCode::BackTab
                if self.tab == Tab::Search && self.focus == Focus::Sidebar =>
            {
                self.query_prompt(self.search_quick);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Sidebar {
                    Focus::Content
                } else {
                    Focus::Sidebar
                };
                if self.tab == Tab::Review {
                    self.switch_tab(Tab::Scm);
                } else if self.tab == Tab::Scm
                    && self.diff.is_some()
                    && self.active_drawer.is_none()
                {
                    self.scm_selection = self.selection;
                    self.switch_tab(Tab::Review);
                }
            }
            KeyCode::Down | KeyCode::Char('j')
                if matches!(self.tab, Tab::Files | Tab::Search) && self.focus == Focus::Content =>
            {
                self.preview_scroll = self.preview_scroll.saturating_add(1).min(
                    self.text
                        .lines()
                        .count()
                        .saturating_sub(1)
                        .min(u16::MAX as usize) as u16,
                );
            }
            KeyCode::Up | KeyCode::Char('k')
                if matches!(self.tab, Tab::Files | Tab::Search) && self.focus == Focus::Content =>
            {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.tab == Tab::Tasks {
                    self.task = (self.task + 1).min(self.tasks.len().saturating_sub(1));
                } else {
                    let count = match self.tab {
                        Tab::Files => self.code.as_ref().map(|c| c.files.len()).unwrap_or(0),
                        Tab::Scm => self.code.as_ref().map(|c| c.changes.len()).unwrap_or(0),
                        Tab::Review => self.diff.as_ref().map(|d| d.rows.len()).unwrap_or(0),
                        Tab::Search => self.search_rows.len(),
                        _ => 0,
                    };
                    self.selection = self
                        .selection
                        .saturating_add(1)
                        .min(count.saturating_sub(1));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.tab == Tab::Tasks {
                    self.task = self.task.saturating_sub(1);
                } else {
                    self.selection = self.selection.saturating_sub(1);
                }
            }
            KeyCode::Char('r') => {
                self.invalidate_quick_index();
                self.refresh()?;
                self.load_tasks()?;
                if let Some(kind) = self.active_drawer {
                    self.load_drawer(kind)?;
                }
            }
            KeyCode::Enter => match self.tab {
                Tab::Tasks => self.choose_execution()?,
                Tab::Files => {
                    self.edit_request = None;
                    let row = self
                        .code
                        .as_ref()
                        .and_then(|c| c.files.get(self.selection))
                        .context("No file selected")?
                        .clone();
                    if row.directory {
                        if let Some(i) = self.expanded.iter().position(|p| p == &row.path) {
                            self.expanded.remove(i);
                        } else {
                            self.expanded.push(row.path);
                        }
                        self.refresh()?;
                    } else {
                        self.open_preview(row.path, None)?;
                    }
                }
                Tab::Scm => self.activate_scm_row()?,
                Tab::Review => {}
                Tab::Search => {
                    if self.search_rows.is_empty() {
                        self.query_prompt(false);
                    } else {
                        self.open_search_result()?;
                    }
                }
            },
            KeyCode::Char('b') => {
                let e = self.execution();
                self.job(move |d| Ok(Data::Bound(d.bind(&e, None)?)))?;
            }
            KeyCode::Char('f') => {
                let e = self.execution();
                self.job(move |d| {
                    d.focus(&e)?;
                    Ok(Data::Business("Focused Agent".into()))
                })?;
            }
            KeyCode::Char('s') if self.tab == Tab::Scm => self.change_stage(true)?,
            KeyCode::Char('u') if self.tab == Tab::Scm => self.change_stage(false)?,
            KeyCode::Char('C') if self.tab == Tab::Scm => {
                self.prepare_commit()?;
            }
            KeyCode::Char('c') if self.tab == Tab::Review => {
                let diff = self.diff.as_ref().context("Open a Diff first")?;
                let row = diff.rows.get(self.selection).context("Select a diff row")?;
                let (side, line) = row
                    .new
                    .map(|n| (Side::New, n))
                    .or_else(|| row.old.map(|n| (Side::Old, n)))
                    .context("Row has no source anchor")?;
                self.prompt = Some(Prompt::Comment {
                    review: self
                        .review
                        .as_ref()
                        .context("Review not loaded")?
                        .id
                        .clone(),
                    anchor: herdr_work_core::CommentAnchor {
                        file: diff.file.clone(),
                        side,
                        start: line,
                        end: line,
                        fingerprint: diff.fingerprint.clone(),
                    },
                });
                self.input.clear();
            }
            KeyCode::Char('s') if self.tab == Tab::Review => {
                let review = self
                    .review
                    .as_ref()
                    .context("No review comments")?
                    .id
                    .clone();
                self.job(move |d| {
                    let outcome = d.send_review(&review)?;
                    Ok(Data::Delivery(
                        d.review(&review)?,
                        format!(
                            "Delivery: {:?}{}",
                            outcome.state,
                            if outcome.focus_warning {
                                " (focus failed; delivery succeeded)"
                            } else {
                                ""
                            }
                        ),
                    ))
                })?;
            }
            KeyCode::Char('n') => {
                self.prompt = Some(Prompt::NewTask);
                self.input.clear();
            }
            KeyCode::Char('d') => {
                let id = self
                    .tasks
                    .get(self.task)
                    .context("No task selected")?
                    .id
                    .clone();
                self.job(move |d| {
                    d.task_action(&id, "delegate")?;
                    Ok(Data::Business("任务已委派".into()))
                })?;
            }
            KeyCode::Char('x') => {
                self.prompt = Some(Prompt::Stop(
                    self.tasks
                        .get(self.task)
                        .context("No task selected")?
                        .id
                        .clone(),
                ));
                self.input.clear();
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn change_stage(&self, stage: bool) -> Result<()> {
        anyhow::ensure!(
            self.scm_rows()
                .get(self.scm_cursor)
                .is_some_and(|r| r.0 == Some(self.selection)),
            "请先选择要暂存或取消暂存的文件"
        );
        let c = self
            .code
            .as_ref()
            .and_then(|c| c.changes.get(self.selection))
            .context("No change selected")?
            .clone();
        self.job(move |d| {
            d.stage(&c, stage)?;
            Ok(Data::Business(
                if stage { "Staged" } else { "Unstaged" }.into(),
            ))
        })
    }
    pub(crate) fn submit_prompt(&mut self) -> Result<()> {
        let input = std::mem::take(&mut self.input);
        let prompt = self.prompt.take().unwrap();
        match prompt {
            Prompt::Git(plan) => {
                anyhow::ensure!(input == "APPLY", "请输入 APPLY 确认 Git 操作");
                self.job(move |d| Ok(Data::Business(d.execute_git_plan(&plan)?)))?;
            }
            Prompt::File(plan) => {
                self.job(move |d| {
                    d.apply_file_action(&plan, &input)?;
                    Ok(Data::Business("文件操作已完成".into()))
                })?;
            }
            Prompt::Folder => self.change_folder(std::path::PathBuf::from(input.trim()))?,
            Prompt::Search(quick) => {
                self.search_query = input;
                self.search_quick = quick;
                self.switch_tab(Tab::Search);
                self.search()?;
            }
            Prompt::SearchInclude => {
                self.search_include = input;
                self.search()?;
            }
            Prompt::SearchExclude => {
                self.search_exclude = input;
                self.search()?;
            }
            Prompt::NewTask => {
                let root = self.root.clone();
                self.job(move |d| {
                    d.create_task(&root, &input)?;
                    Ok(Data::Business("任务已创建".into()))
                })?;
            }
            Prompt::Stop(id) => {
                anyhow::ensure!(
                    input == "STOP",
                    "Type STOP to confirm stopping the selected execution"
                );
                self.job(move |d| {
                    d.task_action(&id, "stop")?;
                    Ok(Data::Business("任务已停止".into()))
                })?;
            }
            Prompt::Commit(plan) => {
                self.job(move |d| Ok(Data::Business(d.commit(&plan, &input)?)))?;
            }
            Prompt::Comment { review, anchor } => {
                self.job(move |d| Ok(Data::Review(d.add_comment_at(&review, &anchor, &input)?)))?;
            }
        }
        Ok(())
    }
}
