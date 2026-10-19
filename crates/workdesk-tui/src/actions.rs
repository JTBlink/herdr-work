//! One terminal owner, with background jobs and upstream-backed views.
use anyhow::{Context as _, Result};
use crossterm::event::KeyCode;
use workdesk_core::Side;

use crate::state::*;
impl App {
    pub(crate) fn key(&mut self, key: KeyCode) -> Result<bool> {
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
            KeyCode::Char('1') => {
                self.tab = Tab::Tasks;
                self.selection = 0;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Files;
                self.selection = 0;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::Scm;
                self.selection = 0;
            }
            KeyCode::Char('4') => {
                self.tab = Tab::Review;
                self.selection = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.tab == Tab::Tasks {
                    self.task = (self.task + 1).min(self.tasks.len().saturating_sub(1));
                } else {
                    let count = match self.tab {
                        Tab::Files => self.code.as_ref().map(|c| c.files.len()).unwrap_or(0),
                        Tab::Scm => self.code.as_ref().map(|c| c.changes.len()).unwrap_or(0),
                        Tab::Review => self.diff.as_ref().map(|d| d.rows.len()).unwrap_or(0),
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
                self.refresh()?;
                self.load_tasks()?;
            }
            KeyCode::Enter => match self.tab {
                Tab::Tasks => self.choose_execution()?,
                Tab::Files => {
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
                        let root = self.root.clone();
                        let file = row.path.strip_prefix(&root)?.to_string_lossy().into_owned();
                        self.job(move |d| Ok(Data::Text(d.file(&root, &file)?)))?;
                    }
                }
                Tab::Scm => {
                    let c = self
                        .code
                        .as_ref()
                        .and_then(|c| c.changes.get(self.selection))
                        .context("No change selected")?
                        .clone();
                    self.open_diff(
                        c.root,
                        c.path,
                        if c.staged { "staged" } else { "unstaged" }.into(),
                    )?;
                }
                Tab::Review => {}
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
                let root = self.root.clone();
                self.job(move |d| Ok(Data::CommitReady(d.prepare_commit(&root)?)))?;
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
                    anchor: workdesk_core::CommentAnchor {
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
