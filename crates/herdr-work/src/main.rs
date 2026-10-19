mod cli;
mod help;
use anyhow::{Context, Result, ensure};
use cli::*;
use herdr_work_core::{BatchId, Execution, ExecutionId, ReviewId, Side, TaskId};
use herdr_work_engine::HerdrWork;

fn print(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn choose(desk: &HerdrWork, args: ExecutionArgs) -> Result<Execution> {
    let execution = if let Some(task) = args.task {
        let tasks = desk.tasks()?;
        let candidates: Vec<_> = tasks
            .into_iter()
            .filter(|t| t.id.0 == task || t.executions.iter().any(|e| e.task.0 == task))
            .flat_map(|t| t.executions)
            .filter(|e| args.execution.as_ref().is_none_or(|id| &e.id.0 == id))
            .collect();
        ensure!(
            candidates.len() == 1,
            "Task has zero or multiple executions; choose --execution explicitly"
        );
        candidates.into_iter().next().unwrap()
    } else {
        let root = args
            .worktree
            .unwrap_or(std::env::current_dir()?)
            .canonicalize()?;
        Execution {
            id: ExecutionId(format!("browse:{}", root.display())),
            task: TaskId("browse".into()),
            attempt: None,
            worktree_identity: HerdrWork::worktree_identity(&root)?,
            worktree: root,
            origin_terminal: None,
            origin_socket: None,
            target: None,
        }
    };
    if args.pane.is_some() {
        desk.bind(&execution, args.pane.as_deref())
    } else {
        desk.bound_execution(&execution)
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("herdr-work: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse_localized();
    let root = HerdrWork::profile_path(cli.home)?;
    if matches!(&cli.command, Some(Command::ProfilePath)) {
        return print(&serde_json::json!({"profile":root}));
    }
    if let Some(Command::Activate {
        action,
        root: worktree,
    }) = &cli.command
    {
        let view = match action.as_str() {
            "show-explorer" => herdr_work_tui::InitialView::Explorer,
            "show-search" => herdr_work_tui::InitialView::Search,
            "show-git" => herdr_work_tui::InitialView::Git,
            "quick-open" => herdr_work_tui::InitialView::QuickOpen,
            _ => unreachable!("clap validates activities"),
        };
        return print(&HerdrWork::activate(&root, worktree.as_deref(), view)?);
    }
    if let Some(Command::SidebarViewer { control }) = &cli.command {
        herdr_work_code::state::configure_product_profile(&root)?;
        let control = control
            .clone()
            .or_else(|| {
                std::env::var_os(herdr_work_code::state::PREVIEW_CONTROL_ENV)
                    .map(std::path::PathBuf::from)
            })
            .context("缺少 viewer 控制路径")?;
        return herdr_work_code::viewer::run(&control).map_err(Into::into);
    }
    if matches!(&cli.command, Some(Command::SidebarEditor)) {
        herdr_work_code::state::configure_product_profile(&root)?;
        return herdr_work_code::actions::run_configured_editor().map_err(Into::into);
    }
    let command = cli.command.unwrap_or(Command::Ui {
        root: None,
        initial_view: None,
        sidebar: false,
        standalone: false,
    });
    if let Command::Ui {
        root: worktree,
        initial_view,
        sidebar: false,
        standalone: false,
    } = &command
        && std::env::var("HERDR_ENV").ok().as_deref() == Some("1")
        && std::env::var("HERDR_PLUGIN_ID").ok().as_deref() != Some("jtblink.herdr-work")
    {
        let view = match initial_view.as_deref() {
            Some(value) => {
                herdr_work_tui::InitialView::from_env_value(value).context("无效的初始活动")?
            }
            None => herdr_work_tui::InitialView::Explorer,
        };
        return print(&HerdrWork::activate(&root, worktree.as_deref(), view)?);
    }
    let desk = HerdrWork::open(&root, cli.projects_backend)?;
    match command {
        Command::Ui {
            root,
            initial_view,
            sidebar,
            standalone,
        } => {
            let initial_view =
                initial_view.or_else(|| std::env::var("HERDR_WORK_INITIAL_VIEW").ok());
            let initial = initial_view
                .as_deref()
                .map(|value| {
                    herdr_work_tui::InitialView::from_env_value(value).context("无效的初始活动")
                })
                .transpose()?;
            let root = root
                .or_else(|| {
                    initial.and_then(|_| {
                        std::env::var_os("HERDR_WORK_ROOT").map(std::path::PathBuf::from)
                    })
                })
                .unwrap_or(std::env::current_dir()?);
            herdr_work_tui::run_with_mode(desk, root, initial, sidebar || !standalone)
        }
        Command::SidebarViewer { .. } | Command::SidebarEditor => {
            unreachable!("internal viewer is dispatched before the profile lock")
        }
        Command::Activate { .. } => {
            unreachable!("host activity is dispatched before the profile lock")
        }
        Command::Doctor => print(
        ),
        Command::ProfilePath => unreachable!("profile path is resolved before opening the profile"),
        Command::Tasks => print(&desk.tasks()?),
        Command::Task { command } => {
            let value = match command {
                TaskCommand::Create { root, title } => desk.create_task(&root, &title)?,
                TaskCommand::Delegate { id } => desk.task_action(&TaskId(id), "delegate")?,
                TaskCommand::Stop { id } => desk.task_action(&TaskId(id), "stop")?,
                TaskCommand::Reopen { id } => desk.task_action(&TaskId(id), "reopen")?,
            };
            print(&value)
        }
        Command::Files { root } => print(&desk.code_snapshot(&root, &[], 0)?),
        Command::Diff { file, scope, base } => {
            print(&desk.diff(&file.worktree, &file.file, &scope, base.as_deref())?)
        }
        Command::Stage { file } => stage(&desk, file, true),
        Command::Unstage { file } => stage(&desk, file, false),
        Command::Commit { worktree, message } => {
            let plan = desk.prepare_commit(&worktree)?;
            print(&serde_json::json!({"commit":desk.commit(&plan,&message)?}))
        }
        Command::Review { command } => match command {
            ReviewCommand::Open {
                execution,
                scope,
                base,
            } => print(&desk.open_review(&choose(&desk, execution)?, &scope, base)?),
            ReviewCommand::Show { id } => print(&desk.review(&ReviewId(id))?),
            ReviewCommand::Add {
                id,
                file,
                line,
                end,
                old,
                text,
            } => print(&desk.add_comment(
                &ReviewId(id),
                &file,
                if old { Side::Old } else { Side::New },
                line,
                end.unwrap_or(line),
                &text,
            )?),
            ReviewCommand::Edit { id, comment, text } => {
                print(&desk.edit_comment(&ReviewId(id), comment, &text)?)
            }
            ReviewCommand::Send { id } => print(&desk.send_review(&ReviewId(id))?),
            ReviewCommand::Resolve {
                id,
                batch,
                delivered,
            } => print(&desk.resolve_batch(&ReviewId(id), &BatchId(batch), delivered)?),
        },
        Command::Focus { execution } => {
            desk.focus(&choose(&desk, execution)?)?;
            print(&serde_json::json!({"ok":true}))
        }
        Command::Reconcile {
            operation,
            evidence,
        } => {
            if let (Some(name), Some(evidence)) = (operation, evidence) {
                print(&desk.resolve_operation(&name, &evidence)?)
            } else {
                print(&desk.reconcile_tasks()?)
            }
        }
        Command::Scheduler { action } => print(&desk.scheduler(&action)?),
    }
}

fn stage(desk: &HerdrWork, file: FileArgs, staged: bool) -> Result<()> {
    let snapshot = desk.code_snapshot(&file.worktree, &[], 0)?;
    let change = snapshot
        .changes
        .iter()
        .find(|c| c.path == file.file && c.staged != staged)
        .context("SCM entry not found")?;
    desk.stage(change, staged)?;
    print(&serde_json::json!({"ok":true}))
}
