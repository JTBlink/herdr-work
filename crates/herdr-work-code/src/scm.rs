//! Shared SCM drawer model extracted from the original Sidebar UI.
use crate::git::Git;
pub struct GitOperation {
    pub args: Vec<String>,
    pub confirm: bool,
}
pub fn reference_operation(reference: &DrawerRef, action: MenuAction) -> Option<GitOperation> {
    let spec = drawer_spec(reference)?;
    let git_spec = drawer_git_spec(reference)?;
    let (args, confirm) = match action {
        MenuAction::Checkout => (drawer_checkout_args(reference, &spec, &git_spec)?, false),
        MenuAction::MergeInto => (
            vec!["merge", "--no-edit", "--", &git_spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            false,
        ),
        MenuAction::CherryPick => (
            vec!["cherry-pick", "--", &git_spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            false,
        ),
        MenuAction::Revert => (
            vec!["revert", "--no-edit", "--", &git_spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            false,
        ),
        MenuAction::StashApply => (
            vec!["stash", "apply", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            false,
        ),
        MenuAction::StashPop => (
            vec!["stash", "pop", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            false,
        ),
        MenuAction::FetchRemote => (
            vec!["fetch", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            false,
        ),
        MenuAction::ResetHere => (drawer_reset_args(&git_spec), true),
        MenuAction::DeleteBranch => (
            vec!["branch", "-d", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            true,
        ),
        MenuAction::StashDrop => (
            vec!["stash", "drop", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            true,
        ),
        MenuAction::DeleteTag => (
            vec!["tag", "-d", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            true,
        ),
        MenuAction::RemoveWorktree => (
            vec!["worktree", "remove", "--", &spec]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            true,
        ),
        _ => return None,
    };
    Some(GitOperation { args, confirm })
}

/// The Git-Graph-style drawers below the Changes section.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Drawer {
    Graph,
    Commits,
    FileHistory,
    Branches,
    Worktrees,
    Remotes,
    Stashes,
    Tags,
}

impl Drawer {
    pub const ALL: [Drawer; 8] = [
        Drawer::Graph,
        Drawer::Commits,
        Drawer::FileHistory,
        Drawer::Branches,
        Drawer::Worktrees,
        Drawer::Remotes,
        Drawer::Stashes,
        Drawer::Tags,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Drawer::Graph => "Graph",
            Drawer::Commits => "Commits",
            Drawer::FileHistory => "File History",
            Drawer::Branches => "Branches",
            Drawer::Worktrees => "Worktrees",
            Drawer::Remotes => "Remotes",
            Drawer::Stashes => "Stashes",
            Drawer::Tags => "Tags",
        }
    }

    pub fn index(self) -> usize {
        Drawer::ALL.iter().position(|d| *d == self).unwrap_or(0)
    }

    /// History views open as documents; repository resources remain drawers.
    pub fn opens_preview(self) -> bool {
        matches!(self, Self::Graph | Self::Commits | Self::FileHistory)
    }
}

/// What a drawer line points at, for clicks and context menus.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum DrawerRef {
    #[default]
    None,
    /// A short commit hash (GRAPH / COMMITS / FILE HISTORY lines).
    Commit(String),
    /// `stash@{n}`.
    Stash(usize),
    Branch {
        name: String,
        current: bool,
    },
    Remote {
        name: String,
        url: String,
    },
    Tag(String),
    /// A worktree's checkout path.
    Worktree(String),
}

/// Parse the actionable reference out of one drawer line.
/// Display form of a `git worktree list` line: the folder NAME plus its
/// branch — the raw absolute path clipped uselessly in a narrow pane.
pub fn pretty_worktree_line(raw: &str) -> String {
    let path = raw.split_whitespace().next().unwrap_or("");
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if let (Some(start), Some(end)) = (raw.find('['), raw.rfind(']'))
        && start < end
    {
        return format!("{name}  ⎇ {}", &raw[start + 1..end]);
    }
    if raw.contains("(bare)") {
        return format!("{name}  (bare)");
    }
    if raw.contains("detached") {
        return format!("{name}  (detached)");
    }
    name.to_string()
}

/// Display form of a remote line: `name  owner/repo` for hosted URLs (the
/// interesting part), the folder name for local-path remotes.
pub fn pretty_remote_line(raw: &str) -> String {
    let mut it = raw.split_whitespace();
    match (it.next(), it.next()) {
        (Some(name), Some(url)) => format!("{name}  {}", pretty_remote_url(url)),
        _ => raw.to_string(),
    }
}

pub fn pretty_remote_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/').trim_end_matches(".git");
    let hosted = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .or_else(|| trimmed.strip_prefix("git@"));
    if let Some(rest) = hosted {
        // git@host:owner/repo and host/owner/repo both → owner/repo.
        let rest = rest.replace(':', "/");
        return match rest.split_once('/') {
            Some((_host, path)) if !path.is_empty() => path.to_string(),
            _ => rest,
        };
    }
    // A local-path remote: its folder name is the recognizable bit.
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

pub fn parse_drawer_ref(kind: Drawer, line: &str) -> DrawerRef {
    match kind {
        Drawer::Graph | Drawer::Commits | Drawer::FileHistory => line
            .split_whitespace()
            .find(|tok| {
                tok.len() >= 7
                    && tok
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            })
            .map(|h| DrawerRef::Commit(h.to_string()))
            .unwrap_or(DrawerRef::None),
        Drawer::Branches => {
            let current = line.starts_with('*');
            let name = line.trim_start_matches('*').trim().to_string();
            if name.is_empty() || name.starts_with('(') {
                DrawerRef::None
            } else {
                DrawerRef::Branch { name, current }
            }
        }
        Drawer::Remotes => {
            let mut it = line.split_whitespace();
            match it.next() {
                Some(name) => DrawerRef::Remote {
                    name: name.to_string(),
                    url: it.next().unwrap_or("").to_string(),
                },
                None => DrawerRef::None,
            }
        }
        Drawer::Worktrees => {
            let path = line.split_whitespace().next().unwrap_or("").to_string();
            if path.is_empty() || path.starts_with('(') {
                DrawerRef::None
            } else {
                DrawerRef::Worktree(path)
            }
        }
        Drawer::Stashes => line
            .strip_prefix("stash@{")
            .and_then(|rest| rest.split('}').next())
            .and_then(|n| n.parse::<usize>().ok())
            .map(DrawerRef::Stash)
            .unwrap_or(DrawerRef::None),
        Drawer::Tags => {
            let name = line.trim().to_string();
            if name.is_empty() || name.starts_with('(') {
                DrawerRef::None
            } else {
                DrawerRef::Tag(name)
            }
        }
    }
}

/// The stable, repo-independent handle a drawer line points at — for mirroring
/// a selection across tabs. `None` for blank graph-edge rows.
pub fn drawer_spec(dref: &DrawerRef) -> Option<String> {
    match dref {
        DrawerRef::Commit(h) => Some(h.clone()),
        DrawerRef::Stash(n) => Some(format!("stash@{{{n}}}")),
        DrawerRef::Branch { name, .. } => Some(name.clone()),
        DrawerRef::Tag(t) => Some(t.clone()),
        DrawerRef::Remote { name, .. } => Some(name.clone()),
        DrawerRef::Worktree(p) => Some(p.clone()),
        DrawerRef::None => None,
    }
}

/// An object name safe to pass to git commands. Tags are fully qualified so
/// their namespace is unambiguous; branch drawer rows can be local or remote,
/// so they stay in git's displayed short form. Every command call site also
/// terminates option parsing before this value.
pub fn drawer_git_spec(dref: &DrawerRef) -> Option<String> {
    match dref {
        DrawerRef::Tag(name) => Some(format!("refs/tags/{name}")),
        _ => drawer_spec(dref),
    }
}

pub fn drawer_checkout_args(dref: &DrawerRef, spec: &str, git_spec: &str) -> Option<Vec<String>> {
    match dref {
        DrawerRef::Branch { .. } => Some(vec![
            "checkout".into(),
            "--end-of-options".into(),
            spec.into(),
        ]),
        DrawerRef::Commit(_) | DrawerRef::Tag(_) => Some(vec![
            "switch".into(),
            "--detach".into(),
            "--".into(),
            git_spec.into(),
        ]),
        _ => None,
    }
}

pub fn drawer_reset_args(git_spec: &str) -> Vec<String> {
    vec![
        "reset".into(),
        "--mixed".into(),
        "--end-of-options".into(),
        git_spec.into(),
        "--".into(),
    ]
}

impl Drawer {
    pub fn load(self, git: &Git, file: Option<&str>, limit: usize) -> Result<Vec<String>, String> {
        match self {
            Self::Graph => git.graph(limit),
            Self::Commits => git.commits(limit),
            Self::FileHistory => match file {
                Some(path) => git.file_history(path, limit),
                None => Ok(vec!["(select a file above)".into()]),
            },
            Self::Branches => git.branches(),
            Self::Worktrees => git.worktrees(),
            Self::Remotes => git.remotes(),
            Self::Stashes => git.stashes(),
            Self::Tags => git.tags(),
        }
    }
}

/// Context-menu actions for file rows and drawer lines.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenDiff,
    StageOrUnstage,
    Discard,
    CopyPath,
    CopyRelativePath,
    OpenExternal,
    Reveal,
    // Drawer-line actions (commits, branches, stashes, remotes, tags).
    ShowRef,
    Checkout,
    MergeInto,
    DeleteBranch,
    CherryPick,
    Revert,
    ResetHere,
    StashApply,
    StashPop,
    StashDrop,
    FetchRemote,
    CopyRef,
    DeleteTag,
    RemoveWorktree,
}

#[derive(Clone, Copy)]
pub enum MenuEntry {
    Action(MenuAction, &'static str),
    Separator,
}

pub fn reference_menu(dref: &DrawerRef) -> Vec<MenuEntry> {
    match dref {
        DrawerRef::Commit(_) => vec![
            MenuEntry::Action(MenuAction::ShowRef, "Show Changes"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::Checkout, "Checkout (Detached)"),
            MenuEntry::Action(MenuAction::CherryPick, "Cherry-Pick"),
            MenuEntry::Action(MenuAction::Revert, "Revert"),
            MenuEntry::Action(MenuAction::ResetHere, "Reset Current Branch Here…"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::CopyRef, "Copy Hash"),
        ],
        DrawerRef::Branch { current: true, .. } => vec![
            MenuEntry::Action(MenuAction::ShowRef, "Show Tip Commit"),
            MenuEntry::Action(MenuAction::CopyRef, "Copy Branch Name"),
        ],
        DrawerRef::Branch { current: false, .. } => vec![
            MenuEntry::Action(MenuAction::Checkout, "Checkout Branch"),
            MenuEntry::Action(MenuAction::MergeInto, "Merge into Current Branch"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::DeleteBranch, "Delete Branch…"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::CopyRef, "Copy Branch Name"),
        ],
        DrawerRef::Stash(_) => vec![
            MenuEntry::Action(MenuAction::ShowRef, "Show Changes"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::StashApply, "Apply Stash"),
            MenuEntry::Action(MenuAction::StashPop, "Pop Stash"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::StashDrop, "Drop Stash…"),
        ],
        DrawerRef::Remote { .. } => vec![
            MenuEntry::Action(MenuAction::FetchRemote, "Fetch"),
            MenuEntry::Action(MenuAction::CopyRef, "Copy URL"),
        ],
        DrawerRef::Tag(_) => vec![
            MenuEntry::Action(MenuAction::ShowRef, "Show Changes"),
            MenuEntry::Action(MenuAction::Checkout, "Checkout Tag"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::DeleteTag, "Delete Tag…"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::CopyRef, "Copy Tag Name"),
        ],
        DrawerRef::Worktree(_) => vec![
            MenuEntry::Action(MenuAction::Reveal, "Reveal in File Explorer"),
            MenuEntry::Action(MenuAction::CopyRef, "Copy Path"),
            MenuEntry::Separator,
            MenuEntry::Action(MenuAction::RemoveWorktree, "Remove Worktree…"),
        ],
        DrawerRef::None => vec![],
    }
}

/// Source viewer git-show query, shared by both frontends.
pub fn show_args(spec: &str, path: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-c".into(),
        "color.ui=always".into(),
        "show".into(),
        "--color=always".into(),
        "--stat".into(),
        "--patch".into(),
        "--no-ext-diff".into(),
        "--end-of-options".into(),
        spec.to_string(),
    ];
    if let Some(p) = path {
        args.push("--".into());
        args.push(p.replace('/', std::path::MAIN_SEPARATOR_STR));
    }
    args
}
