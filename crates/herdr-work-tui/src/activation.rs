//! Typed own-host requests are polled on the existing worker, never injected as keys.
use crate::state::*;
use anyhow::Result;
use herdr_work_engine::{ActivationRequest, InitialView, WorkbenchRegistration};
use std::time::{Duration, Instant};

pub(crate) struct ActivationRuntime {
    registration: WorkbenchRegistration,
    seen: Option<String>,
    request: u64,
    pending: Option<(u64, u64)>,
    last_poll: Instant,
}
impl App {
    pub(crate) fn activate_initial_view(&mut self, view: InitialView) {
        self.interaction.restore = None;
        if self.interaction.restoring_documents {
            self.preview_request = self.preview_request.wrapping_add(1);
            self.interaction.restoring_documents = false;
        }
        self.interaction.pending_scm_key = None;
        self.interaction.user_revision = self.interaction.user_revision.wrapping_add(1);
        match view {
            InitialView::Explorer => self.switch_tab(Tab::Files),
            InitialView::Search => self.query_prompt(false),
            InitialView::Git => self.switch_tab(Tab::Scm),
            InitialView::QuickOpen => self.query_prompt(true),
        }
    }
    pub(crate) fn start_activation(&mut self) -> Result<()> {
        let Some(registration) = self.desk.workbench_registration() else {
            return Ok(());
        };
        let report = registration.clone();
        self.job(move |desk| Ok(Data::ActivationReported(desk.report_workbench(&report))))?;
        self.activation = Some(ActivationRuntime {
            registration,
            seen: None,
            request: 0,
            pending: None,
            last_poll: Instant::now(),
        });
        Ok(())
    }
    pub(crate) fn tick_activation(&mut self, now: Instant) -> Result<()> {
        let Some(runtime) = &mut self.activation else {
            return Ok(());
        };
        if runtime.pending.is_some()
            || now.saturating_duration_since(runtime.last_poll) < Duration::from_millis(500)
        {
            return Ok(());
        }
        runtime.request = runtime.request.wrapping_add(1);
        let request = runtime.request;
        let registration = runtime.registration.clone();
        let seen = runtime.seen.clone();
        runtime.pending = Some((self.context.epoch, request));
        runtime.last_poll = now;
        let submitted = self.job(move |desk| {
            Ok(Data::ActivationRead(
                request,
                desk.pending_activation(&registration, seen.as_deref()),
            ))
        });
        if submitted.is_err() {
            self.activation.as_mut().unwrap().pending = None;
        }
        submitted
    }
    pub(crate) fn activation_read(
        &mut self,
        request: u64,
        result: Result<Option<ActivationRequest>>,
    ) {
        let Some(runtime) = &mut self.activation else {
            return;
        };
        let Some((epoch, pending)) = runtime.pending else {
            return;
        };
        if pending != request {
            return;
        }
        runtime.pending = None;
        match result {
            Ok(Some(activation)) => {
                runtime.seen = Some(activation.nonce.clone());
                if epoch == self.context.epoch {
                    self.apply_activation(activation);
                }
            }
            Ok(None) => {}
            Err(error) if epoch == self.context.epoch => {
                self.status = format!("活动请求读取失败：{error:#}")
            }
            Err(_) => {}
        }
    }
    pub(crate) fn apply_activation(&mut self, activation: ActivationRequest) {
        if self.editor.is_some()
            || self.edit_request.is_some()
            || self.prompt.is_some()
            || self.overlay.is_some()
            || self.git.editing.is_some()
            || self.git.picker.is_some()
        {
            self.status = "已聚焦 HerdrWork；请先结束当前编辑或弹窗，再选择活动入口".into();
            return;
        }
        if activation.root != self.root
            && let Err(error) = self.change_folder(activation.root)
        {
            self.status = error.to_string();
            return;
        }
        self.activate_initial_view(activation.view);
    }
}
