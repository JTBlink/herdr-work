//! Command syntax and Chinese help text.
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "herdr-work",
    version,
    about = "HerdrWork：在一个 Herdr 终端中管理任务、文件、Git 和代码审查",
    after_help = "示例：\n  herdr-work ui .                 打开当前目录\n  herdr-work doctor               检查安装状态\n  herdr-work task create \"修复问题\"   创建任务\n\n运行 herdr-work <子命令> --help 查看详细参数。"
)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    /// 数据目录，默认 HERDR_WORK_HOME 或 ~/.herdr-work
    pub(crate) home: Option<PathBuf>,
    #[arg(long, global = true)]
    /// Projects 后端程序路径，默认使用主程序旁的 herdr-work-projects
    pub(crate) projects_backend: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}
#[derive(Subcommand)]
pub(crate) enum Command {
    /// 启动终端界面，默认浏览当前目录
    Ui {
        #[arg(default_value = ".")]
        /// 仓库或浏览目录，默认当前目录
        root: PathBuf,
    },
    /// 查看版本、数据目录、后端和源码来源
    Doctor,
    #[command(hide = true)]
    ProfilePath,
    /// 列出独立数据目录中的任务
    Tasks,
    /// 创建、委派、停止或重新打开任务
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// 查看文件树与 Git 状态
    Files {
        #[arg(default_value = ".")]
        /// 仓库或浏览目录，默认当前目录
        root: PathBuf,
    },
    /// 查看指定文件的代码差异
    Diff {
        #[command(flatten)]
        file: FileArgs,
        #[arg(long, default_value = "working")]
        /// 差异范围：working、staged、unstaged 或 branch
        scope: String,
        #[arg(long)]
        /// 分支比较基准，如 main 或提交 SHA
        base: Option<String>,
    },
    /// 暂存指定文件的修改
    Stage {
        #[command(flatten)]
        file: FileArgs,
    },
    /// 取消指定文件的暂存
    Unstage {
        #[command(flatten)]
        file: FileArgs,
    },
    /// 提交当前暂存内容
    Commit {
        #[arg(long, default_value = ".")]
        /// Git 工作目录，默认当前目录
        worktree: PathBuf,
        #[arg(short, long)]
        /// Git 提交说明
        message: String,
    },
    /// 创建审查、保存评论和回填 Agent
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    /// 聚焦已验证绑定的 Agent 窗格
    Focus {
        #[command(flatten)]
        execution: ExecutionArgs,
    },
    /// 查看结果未知的操作，或记录核对依据
    Reconcile {
        #[arg(long, requires = "evidence")]
        /// 待核对的操作文件名，必须同时提供 --evidence
        operation: Option<String>,
        #[arg(long, requires = "operation")]
        /// 人工核对依据，必须同时提供 --operation
        evidence: Option<String>,
    },
    /// 查看、暂停或恢复自动调度，不停止 Agent
    Scheduler {
        #[arg(value_parser = ["pause", "resume", "status"])]
        /// status 查看状态，pause 暂停调度，resume 恢复调度
        action: String,
    },
}
#[derive(Args)]
pub(crate) struct FileArgs {
    #[arg(long, default_value = ".")]
    /// Git 工作目录，默认当前目录
    pub(crate) worktree: PathBuf,
    #[arg(long)]
    /// 相对于 Git 工作目录的文件路径
    pub(crate) file: String,
}
#[derive(Args)]
pub(crate) struct ExecutionArgs {
    #[arg(long)]
    /// 任务 ID
    pub(crate) task: Option<String>,
    #[arg(long)]
    /// 执行实例 ID；配合 --task 消除多次执行的歧义
    pub(crate) execution: Option<String>,
    #[arg(long)]
    /// 未指定任务时使用的 Git 工作目录
    pub(crate) worktree: Option<PathBuf>,
    #[arg(long)]
    /// 需要验证并绑定的 Herdr 窗格 ID
    pub(crate) pane: Option<String>,
}
#[derive(Subcommand)]
pub(crate) enum TaskCommand {
    /// 在指定仓库创建任务
    Create {
        #[arg(long, default_value = ".")]
        /// 仓库或浏览目录，默认当前目录
        root: PathBuf,
        /// 任务标题
        title: String,
    },
    /// 委派任务给 Agent
    Delegate {
        /// 任务 ID
        id: String,
    },
    /// 停止任务执行
    Stop {
        /// 任务 ID
        id: String,
    },
    /// 重新打开任务
    Reopen {
        /// 任务 ID
        id: String,
    },
}
#[derive(Subcommand)]
pub(crate) enum ReviewCommand {
    /// 为工作目录或具体执行实例创建审查
    Open {
        #[command(flatten)]
        execution: ExecutionArgs,
        #[arg(long, default_value = "working")]
        /// 差异范围：working、staged、unstaged 或 branch
        scope: String,
        #[arg(long)]
        /// 分支比较基准，如 main 或提交 SHA
        base: Option<String>,
    },
    /// 查看审查与已保存的评论
    Show {
        /// 审查 ID
        id: String,
    },
    /// 添加行级审查评论
    Add {
        /// 审查 ID
        id: String,
        #[arg(long)]
        /// 评论对应的相对文件路径
        file: String,
        #[arg(long)]
        /// 评论起始行，从 1 开始
        line: u32,
        #[arg(long)]
        /// 评论结束行，默认与起始行相同
        end: Option<u32>,
        #[arg(long)]
        /// 将评论锚定到修改前的旧版本
        old: bool,
        /// 评论内容
        text: String,
    },
    /// 修改已保存的评论
    Edit {
        /// 审查 ID
        id: String,
        /// 评论编号
        comment: u64,
        /// 评论内容
        text: String,
    },
    /// 将固定评论批次发送给已验证的 Agent
    Send {
        /// 审查 ID
        id: String,
    },
    /// 手动核对结果未知的发送批次
    Resolve {
        /// 审查 ID
        id: String,
        /// 发送批次 ID
        batch: String,
        #[arg(long)]
        /// 确认已送达；不传则记录为未送达
        delivered: bool,
    },
}
impl Cli {
    pub(crate) fn parse_localized() -> Self {
        let matches = crate::help::command(Self::command()).get_matches();
        Self::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
    }
}
