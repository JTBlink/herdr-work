//! Default Sidebar: drive the migrated source apps with one terminal and
//! one input loop. Host requests remain typed background jobs.
use anyhow::Result;
use crossterm::{
    event::{self, EnableMouseCapture, Event},
    execute,
};
use herdr_work_engine::{ActivationRequest, HerdrWork, InitialView, JobQueue};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

enum HostResult {
    Report(Result<()>),
    Activation(Result<Option<ActivationRequest>>),
    Recovery(Result<(), String>),
    Projects(PathBuf, Result<serde_json::Value>),
    Worktree(PathBuf, Result<()>),
}

pub(crate) fn run(desk: HerdrWork, root: PathBuf, initial: Option<InitialView>) -> Result<()> {
    herdr_work_code::state::configure_product_profile(desk.root())?;
    herdr_work_code::ui::set_color_theme(herdr_work_code::state::load_state().color_theme);
    let registration = desk.ui_registration();
    let jobs = JobQueue::default();
    if let Some(registration) = registration.clone() {
        let desk = desk.clone();
        jobs.submit(0, move || {
            Ok(HostResult::Report(desk.report_ui(&registration)))
        })?;
    }
    crossterm::style::force_color_output(true);
    let mut terminal = ratatui::init();
    let _guard = crate::TerminalGuard;
    herdr_work_code::preview::initialize_terminal_images();
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let saved = herdr_work_code::state::load_state();
    let view = initial.map_or(saved.active, |view| {
        if view == InitialView::Git {
            herdr_work_code::state::View::SourceControl
        } else {
            herdr_work_code::state::View::Explorer
        }
    });
    herdr_work_code::fontsetup::maybe_prompt(&mut terminal, view, saved.merged)?;
    let mut sidebar = herdr_work_code::sidebar_component::Sidebar::new(root, initial);
    if std::env::var("HERDR_WORK_START_COLLAPSED").as_deref() == Ok("1") {
        sidebar.start_collapsed();
    }

    let mut last_poll = Instant::now();
    let mut last_recovery = Instant::now() - Duration::from_secs(3);
    let mut pending = false;
    let mut projects_pending = false;
    let mut worktree_pending = false;
    let mut seen = None;
    let result: Result<()> = (|| {
        loop {
            while let Ok((_, result)) = jobs.results.try_recv() {
                match result {
                    Ok(HostResult::Report(Err(error))) => {
                        sidebar.notice(format!("宿主身份登记失败：{error:#}"))
                    }
                    Ok(HostResult::Report(Ok(()))) => {}
                    Ok(HostResult::Activation(result)) => {
                        pending = false;
                        match result {
                            Ok(Some(request)) => {
                                seen = Some(request.nonce);
                                sidebar.activate(&request.root, request.view);
                            }
                            Err(error) => sidebar.notice(format!("活动读取失败：{error:#}")),
                            _ => {}
                        }
                    }
                    Ok(HostResult::Projects(root, result)) => {
                        projects_pending = false;
                        if root == sidebar.root() {
                            match result {
                                Ok(_) => sidebar.notice("已打开 HerdrWork Projects".into()),
                                Err(error) => {
                                    sidebar.notice(format!("Projects 打开失败：{error:#}"))
                                }
                            }
                        }
                    }
                    Ok(HostResult::Worktree(root, result)) => {
                        worktree_pending = false;
                        if root == sidebar.root() {
                            sidebar.notice(match result {
                                Ok(()) => "已打开 worktree".into(),
                                Err(error) => format!("Worktree 打开失败：{error:#}"),
                            });
                        }
                    }
                    Ok(HostResult::Recovery(Err(error))) => {
                        sidebar.notice(format!("布局恢复未完成：{error}"))
                    }
                    Ok(HostResult::Recovery(Ok(()))) => {}
                    Err(error) => sidebar.notice(error.to_string()),
                }
            }
            terminal.draw(|frame| sidebar.draw(frame))?;
            if event::poll(Duration::from_millis(100))? {
                let event = event::read()?;
                if matches!(event, Event::Key(_) | Event::Mouse(_) | Event::Resize(..))
                    && sidebar.event(event)
                {
                    break;
                }
            }
            if sidebar.take_projects_request() && !projects_pending {
                let desk = desk.clone();
                let root = sidebar.root();
                match jobs.submit(0, move || {
                    let result = desk.projects_command(&["open"], Some(&root));
                    Ok(HostResult::Projects(root, result))
                }) {
                    Ok(()) => {
                        projects_pending = true;
                        sidebar.notice("正在打开 Projects…".into());
                    }
                    Err(error) => sidebar.notice(format!("Projects 打开失败：{error:#}")),
                }
            }
            if let Some((root, path)) = sidebar.take_worktree_request()
                && !worktree_pending
            {
                let desk = desk.clone();
                let source_root = sidebar.root();
                if let Some(socket) = std::env::var_os("HERDR_SOCKET_PATH") {
                    match jobs.submit(0, move || {
                        let result =
                            desk.open_worktree(std::path::Path::new(&socket), &root, &path);
                        Ok(HostResult::Worktree(source_root, result))
                    }) {
                        Ok(()) => {
                            worktree_pending = true;
                            sidebar.notice("正在打开 worktree…".into());
                        }
                        Err(error) => sidebar.notice(format!("Worktree 打开失败：{error:#}")),
                    }
                } else {
                    sidebar.notice("打开 worktree 需要 Herdr 宿主连接".into());
                }
            }
            sidebar.tick();
            if registration.is_some()
                && last_recovery.elapsed() >= Duration::from_secs(2)
                && let Ok(tab) = std::env::var("HERDR_TAB_ID")
                && jobs
                    .submit(0, move || {
                        Ok(HostResult::Recovery(
                            herdr_work_code::takeover::recover_owned_session(&tab),
                        ))
                    })
                    .is_ok()
            {
                last_recovery = Instant::now();
            }

            if !pending
                && last_poll.elapsed() >= Duration::from_millis(500)
                && let Some(registration) = registration.clone()
            {
                let desk = desk.clone();
                let seen = seen.clone();
                if jobs
                    .submit(0, move || {
                        Ok(HostResult::Activation(
                            desk.pending_activation(&registration, seen.as_deref()),
                        ))
                    })
                    .is_ok()
                {
                    pending = true;
                    last_poll = Instant::now();
                }
            }
        }
        Ok(())
    })();
    let saved = sidebar.flush();
    result?;
    anyhow::ensure!(saved, "Sidebar 状态保存失败");
    Ok(())
}
