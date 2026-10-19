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
    Edit,
    SaveEditor,
    DiscardEditor,
    Cancel,
    Drawer(herdr_work_code::scm::Drawer),
    Search(bool),
    FileAction(actions::MenuAction),
    Reference(herdr_work_code::scm::MenuAction),
}
pub(crate) enum Overlay {
    Settings(usize),
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
        IconTheme::from_state_name(&self.preferences.icon_theme).unwrap_or(IconTheme::Emoji)
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
            format!("HerdrWork           v{}", env!("CARGO_PKG_VERSION")),
        ]
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
            && let crate::scm::ScmTarget::History(kind, index) = entry.3
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
                    for entry in actions::menu_entries(Some(file.directory), false) {
                        if let actions::MenuEntry::Action(action, label) = entry {
                            let mapped = match action {
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
                                _ => None,
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
                if row {
                    rows.push((Command::Key(KeyCode::Enter), "查看差异"));
                    if let Some(change) = self
                        .code
                        .as_ref()
                        .and_then(|c| c.changes.get(self.selection))
                    {
                        rows.push((
                            Command::Key(KeyCode::Char(if change.staged { 'u' } else { 's' })),
                            if change.staged {
                                "取消暂存"
                            } else {
                                "暂存更改"
                            },
                        ));
                        rows.push((Command::Copy(true), "复制相对路径"));
                    }
                }
                rows.push((Command::Key(KeyCode::Char('C')), "提交暂存更改"));
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
            3 => p.icon_theme = self.icon_theme().toggled().state_name().into(),
            4 => p.show_hidden = !p.show_hidden,
            5 => p.git_decorations = !p.git_decorations,
            6 => p.show_hotkeys = !p.show_hotkeys,
            _ => return Ok(()),
        }
        self.desk.save_ui_preferences(&p)?;
        let refresh = p.show_hidden != self.preferences.show_hidden;
        self.preferences = p;
        ui::set_color_theme(self.color_theme());
        self.status = "设置已保存".into();
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
                self.overlay = Some(Overlay::Settings(index));
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
            let c = self
                .code
                .as_ref()
                .and_then(|c| c.changes.get(self.selection))
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
            Command::Settings => self.open_settings(),
            Command::Collapse => {
                self.expanded.clear();
                self.selection = 0;
                self.refresh()?;
            }
            Command::Copy(relative) => {
                let path = self.selected_path()?;
                let path = if relative {
                    path.strip_prefix(&self.root).unwrap_or(&path)
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
                self.save_editor()?;
                self.finish_editor()?;
            }
            Command::DiscardEditor => self.finish_editor()?,
            Command::Cancel => {}
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
                    _ => self.job(move |d| {
                        Ok(Data::GitReady(d.prepare_scm_action(&view, index, action)?))
                    })?,
                }
            }
            Command::Search(quick) => self.query_prompt(quick),
            Command::FileAction(action) => match action {
                actions::MenuAction::ChangeFolderTyped => {
                    self.prompt = Some(Prompt::Folder);
                    self.input = self.root.to_string_lossy().into_owned();
                }
                actions::MenuAction::ChangeFolder => {
                    if cfg!(target_os = "linux") {
                        self.prompt = Some(Prompt::Folder);
                        self.input = self.root.to_string_lossy().into_owned();
                    } else {
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
                    Overlay::Settings(i) => *i = index,
                    Overlay::Menu { selected, .. } => *selected = index,
                }
                self.overlay_key(KeyCode::Enter)?;
            }
        }
        Ok(())
    }
}
