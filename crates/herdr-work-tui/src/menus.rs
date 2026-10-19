//! Visible settings and context menus use the same commands as keyboard input.
use crate::state::*;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use herdr_work_code::{actions, appearance::ColorTheme, icons::IconTheme, ui};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Clone, Copy)]
pub(crate) enum Command {
    Key(KeyCode),
    Copy(bool),
    Collapse,
    External,
    Reveal,
    Settings,
    FontHelp,
    Edit,
    SaveEditor,
    DiscardEditor,
    ReloadEditor,
    OverwriteEditor,
    Cancel,
    Drawer(herdr_work_code::scm::Drawer),
    Search(bool),
    FileAction(actions::MenuAction),
    Reference(herdr_work_code::scm::MenuAction),
    Recovery,
}
pub(crate) enum Overlay {
    Settings(usize),
    Recovery(usize),
    Menu {
        title: &'static str,
        rows: Vec<(Command, &'static str)>,
        selected: usize,
    },
}
pub(crate) fn modal_rect(area: Rect, rows: usize) -> Rect {
    let width = area.width.saturating_sub(2).min(60);
    let height = area.height.saturating_sub(2).min(rows as u16 + 4);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
impl App {
    pub(crate) fn icon_theme(&self) -> IconTheme {
        IconTheme::from_state_name(&self.preferences.icon_theme)
            .or(self.interaction.auto_icons)
            .unwrap_or(IconTheme::Emoji)
    }
    pub(crate) fn color_theme(&self) -> ColorTheme {
        ColorTheme::from_state_name(&self.preferences.color_theme).unwrap_or(ColorTheme::VsCode)
    }
    pub(crate) fn settings_rows(&self) -> Vec<String> {
        let p = &self.preferences;
        let on = |v| if v { "开启" } else { "关闭" };
        vec![
            format!("侧栏宽度            {} 列  ←/→", p.sidebar_width),
            format!("靠右停靠            {}", on(p.dock_right)),
            format!("颜色主题            {}", p.color_theme),
            format!("图标主题            {}", p.icon_theme),
            format!("显示隐藏文件        {}", on(p.show_hidden)),
            format!("Git 状态装饰        {}", on(p.git_decorations)),
            format!("底部快捷键          {}", on(p.show_hotkeys)),
            format!("预览选区自动复制    {}", on(p.copy_on_select)),
            format!("Git 分支与同步栏     {}", on(p.show_git_footer)),
            "字体使用说明（手动选择 Nerd Font）".into(),
            format!("HerdrWork           v{}", env!("CARGO_PKG_VERSION")),
        ]
    }
    fn font_help(&mut self) {
        self.overlay = Some(Overlay::Menu {
            title: "字体与图标",
            rows: vec![
                (Command::Cancel, "Emoji 图标适合普通终端字体"),
                (Command::Cancel, "Material 图标需终端手动选择 Nerd Font"),
                (Command::Cancel, "auto 只检测已安装字体；可随时切回 Emoji"),
                (Command::Cancel, "字体资料：https://www.nerdfonts.com/"),
                (Command::Cancel, "在当前终端设置中选择字体后重新打开工作台"),
                (Command::Cancel, "关闭说明"),
            ],
            selected: 0,
        });
    }
    pub(crate) fn open_settings(&mut self) {
        self.edit_request = None;
        self.overlay = Some(Overlay::Settings(0));
    }
    pub(crate) fn open_menu(&mut self, row: bool) {
        self.edit_request = None;
        self.menu_path = if row { self.selected_path().ok() } else { None };
        if row
            && self.tab == Tab::Scm
            && let Some(entry) = self.scm_rows().get(self.scm_cursor)
            && let crate::scm::ScmTarget::History(kind, index) = entry.3.clone()
            && let Some(view) = self.drawers.get(&kind)
            && let Some(line) = view.lines.get(index)
        {
            use herdr_work_code::scm::{MenuEntry, parse_drawer_ref, reference_menu};
            let reference = parse_drawer_ref(kind, line);
            let rows = reference_menu(&reference)
                .into_iter()
                .filter_map(|e| {
                    if let MenuEntry::Action(action, label) = e {
                        Some((Command::Reference(action), label))
                    } else {
                        None
                    }
                })
                .collect();
            self.menu_reference = Some((view.clone(), index));
            self.overlay = Some(Overlay::Menu {
                title: "Git 对象菜单",
                rows,
                selected: 0,
            });
            return;
        }
        let mut rows = vec![];
        let title = match self.tab {
            Tab::Files => {
                if row
                    && let Some(file) = self.code.as_ref().and_then(|c| c.files.get(self.selection))
                {
                    rows.push((
                        Command::Key(KeyCode::Enter),
                        if file.directory {
                            "展开/折叠目录"
                        } else {
                            "预览文件"
                        },
                    ));
                    if !file.directory {
                        rows.push((Command::Edit, "编辑文本"));
                    }
                    // The original menu model and platform actions remain authoritative.
                    for entry in actions::menu_entries(Some(file.directory), true) {
                        if let actions::MenuEntry::Action(action, label) = entry {
                            let mapped = match action {
                                actions::MenuAction::Stage => {
                                    Some((Command::Key(KeyCode::Char('s')), "暂存文件/目录更改"))
                                }
                                actions::MenuAction::CopyPath => {
                                    Some((Command::Copy(false), "复制绝对路径"))
                                }
                                actions::MenuAction::CopyRelativePath => {
                                    Some((Command::Copy(true), "复制相对路径"))
                                }
                                actions::MenuAction::OpenExternal => {
                                    Some((Command::External, "使用默认应用打开"))
                                }
                                actions::MenuAction::Reveal => {
                                    Some((Command::Reveal, "在文件管理器显示"))
                                }
                                actions::MenuAction::NewFile => {
                                    Some((Command::FileAction(action), "新建文件"))
                                }
                                actions::MenuAction::NewFolder => {
                                    Some((Command::FileAction(action), "新建文件夹"))
                                }
                                actions::MenuAction::Rename => {
                                    Some((Command::FileAction(action), "重命名"))
                                }
                                actions::MenuAction::Delete => {
                                    Some((Command::FileAction(action), "删除…"))
                                }
                                actions::MenuAction::ChangeFolder => {
                                    Some((Command::FileAction(action), "打开文件夹…"))
                                }
                                actions::MenuAction::ChangeFolderTyped => {
                                    Some((Command::FileAction(action), "输入文件夹路径…"))
                                }
                            };
                            let _ = label;
                            if let Some(item) = mapped {
                                rows.push(item);
                            }
                        }
                    }
                }
                if !row {
                    rows.extend([
                        (
                            Command::FileAction(actions::MenuAction::NewFile),
                            "新建文件",
                        ),
                        (
                            Command::FileAction(actions::MenuAction::NewFolder),
                            "新建文件夹",
                        ),
                        (
                            Command::FileAction(actions::MenuAction::ChangeFolderTyped),
                            "打开文件夹…",
                        ),
                    ]);
                }
                rows.push((Command::Collapse, "折叠全部目录"));
                "文件菜单"
            }
            Tab::Scm | Tab::Review => {
                if row
                    && self
                        .scm_rows()
                        .get(self.scm_cursor)
                        .is_some_and(|r| matches!(r.3, crate::scm::ScmTarget::Change(_)))
                {
                    rows.push((Command::Key(KeyCode::Enter), "查看差异"));
                    if let Some(change) = self.code.as_ref().and_then(|c| {
                        self.scm_rows()
                            .get(self.scm_cursor)
                            .and_then(|r| r.0)
                            .and_then(|index| c.changes.get(index))
                    }) {
                        rows.push((
                            Command::Key(KeyCode::Char(if change.staged { 'u' } else { 's' })),
                            if change.staged {
                                "取消暂存"
                            } else {
                                "暂存更改"
                            },
                        ));
                        if !change.staged {
                            rows.push((Command::Key(KeyCode::Char('d')), "丢弃文件更改…"));
                        }
                        rows.extend([
                            (Command::External, "使用默认应用打开"),
                            (Command::Reveal, "在文件管理器显示"),
                            (Command::Copy(false), "复制绝对路径"),
                            (Command::Copy(true), "复制相对路径"),
                        ]);
                    }
                }
                if row && let Some(entry) = self.scm_rows().get(self.scm_cursor) {
                    match entry.3 {
                        crate::scm::ScmTarget::Directory(_, staged, _)
                        | crate::scm::ScmTarget::Section(_, staged) => rows.push((
                            Command::Key(KeyCode::Char(if staged { 'u' } else { 's' })),
                            if staged {
                                "取消暂存此目录/分组"
                            } else {
                                "暂存此目录/分组"
                            },
                        )),
                        crate::scm::ScmTarget::Repository(_) => {
                            rows.push((Command::Key(KeyCode::Char('s')), "暂存此仓库"));
                            rows.push((Command::Key(KeyCode::Char('u')), "取消暂存此仓库"));
                        }
                        _ => {}
                    }
                }
                rows.extend([
                    (Command::Key(KeyCode::Char('C')), "提交暂存更改"),
                    (Command::Key(KeyCode::Char('A')), "生成提交消息建议"),
                    (Command::Key(KeyCode::Char('a')), "暂存当前仓库全部更改…"),
                    (Command::Key(KeyCode::Char('U')), "取消当前仓库全部暂存…"),
                    (
                        Command::Key(KeyCode::Char('D')),
                        "丢弃当前仓库全部未暂存更改…",
                    ),
                    (
                        Command::Key(KeyCode::Char('z')),
                        "创建 Stash（包含 untracked）…",
                    ),
                    (Command::Key(KeyCode::Char('t')), "切换列表/目录树"),
                    (Command::Key(KeyCode::Char('B')), "选择/创建分支…"),
                    (
                        Command::Key(KeyCode::Char('S')),
                        "同步当前仓库（pull rebase + push）…",
                    ),
                    (Command::Recovery, "核对 Unknown 操作并显式恢复…"),
                ]);
                for kind in herdr_work_code::scm::Drawer::ALL {
                    rows.push((Command::Drawer(kind), kind.title()));
                }
                "Git 菜单"
            }
            Tab::Tasks => {
                rows.push((Command::Key(KeyCode::Char('n')), "创建任务"));
                "任务菜单"
            }
            Tab::Search => {
                rows.push((Command::Search(false), "搜索文本"));
                rows.push((Command::Search(true), "快速打开文件"));
                if row && !self.search_rows.is_empty() {
                    rows.push((Command::Key(KeyCode::Enter), "预览文件"));
                    rows.push((Command::Edit, "编辑文本"));
                }
                "搜索菜单"
            }
        };
        rows.extend([
            (Command::Key(KeyCode::Char('r')), "刷新"),
            (Command::Settings, "侧栏设置"),
            (Command::FontHelp, "字体与图标说明"),
        ]);
        self.overlay = Some(Overlay::Menu {
            title,
            rows,
            selected: 0,
        });
    }
    fn toggle_setting(&mut self, index: usize, wider: bool) -> Result<()> {
        let mut p = self.preferences.clone();
        match index {
            0 => {
                p.sidebar_width =
                    herdr_work_code::appearance::step_sidebar_width(p.sidebar_width, wider)
            }
            1 => p.dock_right = !p.dock_right,
            2 => p.color_theme = self.color_theme().next().label().into(),
            3 => {
                p.icon_theme = match p.icon_theme.as_str() {
                    "emoji" => "material",
                    "material" => "auto",
                    _ => "emoji",
                }
                .into()
            }
            4 => p.show_hidden = !p.show_hidden,
            5 => p.git_decorations = !p.git_decorations,
            6 => p.show_hotkeys = !p.show_hotkeys,
            7 => p.copy_on_select = !p.copy_on_select,
            8 => p.show_git_footer = !p.show_git_footer,
            9 => {
                self.font_help();
                return Ok(());
            }
            _ => return Ok(()),
        }
        let refresh = p.show_hidden != self.preferences.show_hidden
            || p.git_decorations != self.preferences.git_decorations;
        self.preferences = p;
        ui::set_color_theme(self.color_theme());
        self.save_preferences()?;
        self.status = "正在保存设置…".into();
        if refresh {
            self.refresh()?;
        }
        Ok(())
    }
    pub(crate) fn overlay_key(&mut self, key: KeyCode) -> Result<()> {
        let Some(overlay) = self.overlay.take() else {
            return Ok(());
        };
        if key == KeyCode::Esc {
            return Ok(());
        }
        match overlay {
            Overlay::Recovery(mut index) => {
                match key {
                    KeyCode::Up => index = index.saturating_sub(1),
                    KeyCode::Down => {
                        index = (index + 1).min(self.git.unresolved.len().saturating_sub(1))
                    }
                    KeyCode::Enter => {
                        if let Some((name, _)) = self.git.unresolved.get(index) {
                            self.prompt = Some(Prompt::Resolve(name.clone()));
                            self.input.clear();
                            return Ok(());
                        }
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Recovery(index));
            }
            Overlay::Settings(mut index) => {
                match key {
                    KeyCode::Up => index = index.saturating_sub(1),
                    KeyCode::Down => index = (index + 1).min(self.settings_rows().len() - 1),
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right => {
                        self.toggle_setting(index, true)?;
                    }
                    KeyCode::Left => {
                        self.toggle_setting(index, false)?;
                    }
                    _ => {}
                }
                if self.overlay.is_none() {
                    self.overlay = Some(Overlay::Settings(index));
                }
            }
            Overlay::Menu {
                title,
                rows,
                mut selected,
            } => {
                match key {
                    KeyCode::Up => selected = selected.saturating_sub(1),
                    KeyCode::Down => selected = (selected + 1).min(rows.len().saturating_sub(1)),
                    KeyCode::Enter => {
                        if let Some((command, _)) = rows.get(selected) {
                            return self.menu_command(*command);
                        }
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Menu {
                    title,
                    rows,
                    selected,
                });
            }
        }
        Ok(())
    }
    pub(crate) fn selected_path(&self) -> Result<std::path::PathBuf> {
        if self.tab == Tab::Search {
            return Ok(self
                .search_rows
                .get(self.selection)
                .context("未选择搜索结果")?
                .path
                .clone());
        }
        if self.tab == Tab::Files {
            Ok(self
                .code
                .as_ref()
                .and_then(|c| c.files.get(self.selection))
                .context("未选择文件")?
                .path
                .clone())
        } else {
            let index = self
                .scm_rows()
                .get(self.scm_cursor)
                .and_then(|r| r.0)
                .context("未选择 Git 文件")?;
            let c = self
                .code
                .as_ref()
                .and_then(|c| c.changes.get(index))
                .context("未选择 Git 更改")?;
            Ok(c.root.join(&c.path))
        }
    }
    fn menu_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Key(key) => {
                if self.tab == Tab::Review {
                    self.switch_tab(Tab::Scm);
                }
                self.action(key)?;
            }
            Command::Recovery => {
                self.git.request += 1;
                let request = self.git.request;
                self.job(move |d| Ok(Data::Reconcile(request, d.unresolved_git_operations())))?;
            }
            Command::Settings => self.open_settings(),
            Command::FontHelp => self.font_help(),
            Command::Collapse => {
                self.expanded.clear();
                self.selection = 0;
                self.refresh()?;
            }
            Command::Copy(relative) => {
                let path = self.selected_path()?;
                let relative_root = if matches!(self.tab, Tab::Scm | Tab::Review) {
                    self.selected_repository()
                } else {
                    self.root.clone()
                };
                let path = if relative {
                    path.strip_prefix(&relative_root).unwrap_or(&path)
                } else {
                    &path
                };
                self.status = match actions::copy_to_clipboard(&path.to_string_lossy())? {
                    actions::ClipboardWrite::Native => "路径已复制",
                    actions::ClipboardWrite::Osc52Unacknowledged => "已发送到终端剪贴板",
                }
                .into();
            }
            Command::External => actions::open_external(&self.selected_path()?)?,
            Command::Reveal => {
                let p = self.selected_path()?;
                actions::reveal(&p, p.is_dir());
            }
            Command::Edit => self.start_editor()?,
            Command::SaveEditor => {
                self.save_editor_with(false, true)?;
            }
            Command::DiscardEditor => self.finish_editor()?,
            Command::ReloadEditor => self.reload_editor()?,
            Command::OverwriteEditor => {
                self.save_editor_with(true, self.editor_leave_after_save)?
            }
            Command::Cancel => {
                self.editor_leave_after_save = false;
            }
            Command::Reference(action) => {
                use herdr_work_code::scm::{DrawerRef, MenuAction, drawer_spec, parse_drawer_ref};
                let (view, index) = self
                    .menu_reference
                    .as_ref()
                    .context("未选择 Git 对象")?
                    .clone();
                let line = view.lines.get(index).context("Git 对象已失效")?;
                let reference = parse_drawer_ref(view.kind, line);
                match action {
                    MenuAction::ShowRef => {
                        self.drawer_request += 1;
                        let request = self.drawer_request;
                        self.job(move |d| Ok(Data::Show(request, d.scm_show(&view, index))))?;
                    }
                    MenuAction::CopyRef => {
                        let text = match &reference {
                            DrawerRef::Remote { url, .. } => url.clone(),
                            _ => drawer_spec(&reference).context("没有可复制的引用")?,
                        };
                        actions::copy_to_clipboard(&text)?;
                        self.status = "Git 引用已复制或发送到终端剪贴板".into();
                    }
                    MenuAction::Reveal => {
                        let path = drawer_spec(&reference).context("没有工作目录路径")?;
                        actions::reveal(std::path::Path::new(&path), true);
                    }
                    MenuAction::DeleteBranch => {
                        if let DrawerRef::Branch { name, .. } = reference {
                            self.git.active = Some(view.root.clone());
                            self.work_git(herdr_work_engine::WorkGitAction::DeleteBranch(name))?;
                        }
                    }
                    MenuAction::Checkout if matches!(reference, DrawerRef::Branch { .. }) => {
                        if let DrawerRef::Branch { name, .. } = reference {
                            self.git.active = Some(view.root.clone());
                            self.git.request += 1;
                            let request = self.git.request;
                            self.job(move |d| {
                                Ok(Data::WorkGitReady(
                                    request,
                                    d.prepare_branch_checkout(&view.root, &name),
                                ))
                            })?;
                        }
                    }
                    _ => {
                        self.git.request += 1;
                        let request = self.git.request;
                        self.job(move |d| {
                            Ok(Data::GitReady(
                                request,
                                d.prepare_scm_action(&view, index, action),
                            ))
                        })?;
                    }
                }
            }
            Command::Search(quick) => self.query_prompt(quick),
            Command::FileAction(action) => match action {
                actions::MenuAction::ChangeFolderTyped => {
                    self.prompt = Some(Prompt::Folder);
                    self.input = self.root.to_string_lossy().into_owned();
                }
                actions::MenuAction::ChangeFolder => {
                    #[cfg(not(any(windows, target_os = "macos")))]
                    {
                        self.prompt = Some(Prompt::Folder);
                        self.input = self.root.to_string_lossy().into_owned();
                    }
                    #[cfg(any(windows, target_os = "macos"))]
                    {
                        let root = self.root.clone();
                        self.job(move |_| Ok(Data::Folder(actions::pick_folder(&root))))?;
                    }
                }
                _ => {
                    let root = self
                        .code
                        .as_ref()
                        .map(|c| c.root.clone())
                        .unwrap_or_else(|| self.root.clone());
                    let target = self.menu_path.clone();
                    self.job(move |d| {
                        Ok(Data::FileReady(d.prepare_file_action(
                            &root,
                            target.as_deref(),
                            action,
                        )?))
                    })?;
                }
            },
            Command::Drawer(kind) => {
                self.switch_tab(Tab::Scm);
                self.load_drawer(kind)?;
            }
        }
        Ok(())
    }
    pub(crate) fn draw_overlay(&self, f: &mut Frame, area: Rect) {
        let Some(overlay) = &self.overlay else {
            return;
        };
        let (title, rows, selected) = match overlay {
            Overlay::Recovery(i) => (
                "Unknown 核对（Enter 填写证据；不会重试）",
                if self.git.unresolved.is_empty() {
                    vec!["没有待核对的操作".into()]
                } else {
                    self.git
                        .unresolved
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect()
                },
                *i,
            ),
            Overlay::Settings(i) => ("侧栏设置", self.settings_rows(), *i),
            Overlay::Menu {
                title,
                rows,
                selected,
            } => (
                *title,
                rows.iter().map(|r| r.1.to_string()).collect(),
                *selected,
            ),
        };
        let rect = modal_rect(area, rows.len());
        f.render_widget(Clear, rect);
        let block = Block::default().borders(Borders::ALL).title(title).style(
            ratatui::style::Style::default()
                .bg(ui::palette().keycap_bg)
                .fg(ui::palette().selection_fg),
        );
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let offset = selected.saturating_sub(inner.height.saturating_sub(3) as usize);
        let items = rows
            .into_iter()
            .enumerate()
            .skip(offset)
            .map(|(i, text)| {
                ListItem::new(text).style(if i == selected {
                    ui::selection_style(true)
                } else {
                    ratatui::style::Style::default()
                })
            })
            .collect::<Vec<_>>();
        f.render_widget(
            List::new(items),
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(1),
            ),
        );
        f.render_widget(
            Paragraph::new("↑/↓ 选择 · Enter 操作 · Esc 返回"),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
    }
    pub(crate) fn overlay_mouse(&mut self, event: MouseEvent, area: Rect) -> Result<()> {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return Ok(());
        }
        let Some(overlay) = &self.overlay else {
            return Ok(());
        };
        let (count, selected) = match overlay {
            Overlay::Recovery(i) => (self.git.unresolved.len(), *i),
            Overlay::Settings(i) => (self.settings_rows().len(), *i),
            Overlay::Menu { rows, selected, .. } => (rows.len(), *selected),
        };
        let rect = modal_rect(area, count);
        if !rect.contains(Position::new(event.column, event.row)) {
            self.overlay = None;
            return Ok(());
        }
        let inner = Block::default().borders(Borders::ALL).inner(rect);
        if event.row >= inner.y && event.row < inner.bottom().saturating_sub(1) {
            let offset = selected.saturating_sub(inner.height.saturating_sub(3) as usize);
            let index = event.row.saturating_sub(inner.y) as usize + offset;
            if index < count {
                match self.overlay.as_mut().unwrap() {
                    Overlay::Recovery(i) => *i = index,
                    Overlay::Settings(i) => *i = index,
                    Overlay::Menu { selected, .. } => *selected = index,
                }
                self.overlay_key(KeyCode::Enter)?;
            }
        }
        Ok(())
    }
}
