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
    let desk = HerdrWork::open(&root, cli.projects_backend)?;
    match cli.command.unwrap_or(Command::Ui {
        root: std::env::current_dir()?,
    }) {
        Command::Ui { root } => herdr_work_tui::run(desk, root),
        Command::Doctor => print(
            &serde_json::json!({"product":"HerdrWork","version":env!("CARGO_PKG_VERSION"),"profile":desk.root(),"projects_backend":desk.backend(),"projects_backend_installed":desk.backend().is_file(),"projects_backend_platform":if cfg!(unix){"bundled"}else{"Unix backend required; native Windows task execution unavailable"},"sources":{"projects":"92a2813be9a7c2f8cda737f6b78ed21e8413c3ce","sidebar":"081ed133f3faa377438b51ce435fe8a87c439593","reviewr":"859dd5b11a5280fe282de56b52378b20a54fa10a"}}),
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
        Command::Import { source, apply } => {
            let preview = desk.import_preview(&source)?;
            if apply {
                desk.import_projects(&preview)?;
            }
            print(&preview)
        }
        Command::RollbackImport => {
            desk.rollback_import()?;
            print(&serde_json::json!({"ok":true}))
        }
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
