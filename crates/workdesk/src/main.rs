use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use workdesk_core::{BatchId, Execution, ExecutionId, ReviewId, Side, TaskId};
use workdesk_engine::Workdesk;

#[derive(Parser)]
#[command(
    name = "workdesk",
    version,
    about = "Tasks, files, Git and review in one Herdr workdesk"
)]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    projects_backend: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    Ui {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    Doctor,
    Tasks,
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    Files {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    Diff {
        #[command(flatten)]
        file: FileArgs,
        #[arg(long, default_value = "working")]
        scope: String,
        #[arg(long)]
        base: Option<String>,
    },
    Stage {
        #[command(flatten)]
        file: FileArgs,
    },
    Unstage {
        #[command(flatten)]
        file: FileArgs,
    },
    Commit {
        #[arg(long, default_value = ".")]
        worktree: PathBuf,
        #[arg(short, long)]
        message: String,
    },
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    Focus {
        #[command(flatten)]
        execution: ExecutionArgs,
    },
    Reconcile {
        #[arg(long, requires = "evidence")]
        operation: Option<String>,
        #[arg(long, requires = "operation")]
        evidence: Option<String>,
    },
    Scheduler {
        #[arg(value_parser = ["pause", "resume", "status"])]
        action: String,
    },
    Import {
        source: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    RollbackImport,
}
#[derive(Args)]
struct FileArgs {
    #[arg(long, default_value = ".")]
    worktree: PathBuf,
    #[arg(long)]
    file: String,
}
#[derive(Args)]
struct ExecutionArgs {
    #[arg(long)]
    task: Option<String>,
    #[arg(long)]
    execution: Option<String>,
    #[arg(long)]
    worktree: Option<PathBuf>,
    #[arg(long)]
    pane: Option<String>,
}
#[derive(Subcommand)]
enum TaskCommand {
    Create {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        title: String,
    },
    Delegate {
        id: String,
    },
    Stop {
        id: String,
    },
    Reopen {
        id: String,
    },
}
#[derive(Subcommand)]
enum ReviewCommand {
    Open {
        #[command(flatten)]
        execution: ExecutionArgs,
        #[arg(long, default_value = "working")]
        scope: String,
        #[arg(long)]
        base: Option<String>,
    },
    Show {
        id: String,
    },
    Add {
        id: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        line: u32,
        #[arg(long)]
        end: Option<u32>,
        #[arg(long)]
        old: bool,
        text: String,
    },
    Edit {
        id: String,
        comment: u64,
        text: String,
    },
    Send {
        id: String,
    },
    Resolve {
        id: String,
        batch: String,
        #[arg(long)]
        delivered: bool,
    },
}
fn print(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn choose(desk: &Workdesk, args: ExecutionArgs) -> Result<Execution> {
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
            worktree_identity: Workdesk::worktree_identity(&root)?,
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
        eprintln!("workdesk: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    let desk = Workdesk::open(
        &cli.home.unwrap_or_else(Workdesk::default_root),
        cli.projects_backend,
    )?;
    match cli.command.unwrap_or(Command::Ui {
        root: std::env::current_dir()?,
    }) {
        Command::Ui { root } => workdesk_tui::run(desk, root),
        Command::Doctor => print(
        ),
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

fn stage(desk: &Workdesk, file: FileArgs, staged: bool) -> Result<()> {
    let snapshot = desk.code_snapshot(&file.worktree, &[], 0)?;
    let change = snapshot
        .changes
        .iter()
        .find(|c| c.path == file.file && c.staged != staged)
        .context("SCM entry not found")?;
    desk.stage(change, staged)?;
    print(&serde_json::json!({"ok":true}))
}
