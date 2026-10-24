//! Chinese settings presentation shared by Explorer and Source Control.
//! Persisted setting names and values remain the source application's tokens.
use crate::ui::truncate_to;
use ratatui::text::Span;

pub fn close_button(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    if area.width < 4 || area.height == 0 {
        return ratatui::layout::Rect::default();
    }
    ratatui::layout::Rect::new(area.right() - 4, area.y, 3, 1)
}

pub fn draw_close(frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    frame.render_widget(
        ratatui::widgets::Paragraph::new(" × ").style(crate::ui::chrome_button_style(false)),
        close_button(area),
    );
}

pub fn text(value: &str) -> &str {
    match value {
        "HerdrWork version" => "HerdrWork 版本",
        "Sidebar version" => "Sidebar 版本",
        "Unified sidebar" => "合并侧栏",
        "Dock on the right" => "右侧停靠",
        "Sidebar width" => "侧栏宽度",
        "Icon theme" => "图标主题",
        "Color theme" => "颜色主题",
        "Preview opens in" => "文件打开方式",
        "Open in VS Code" => "通过 VS Code 打开",
        "Settings scope" => "配置范围",
        "HerdrWork only" => "仅 HerdrWork",
        "Current profile" => "当前资料目录",
        "Custom editor..." => "自定义编辑器…",
        "Use editor on click" => "单击使用编辑器",
        "Hidden files" => "隐藏文件",
        "Footer hotkeys" => "底部快捷键",
        "Auto-open sidebar" => "自动打开侧栏",
        "Strict toggle" => "切换时直接关闭",
        "Focus on open" => "打开后聚焦",
        "Follow pane folder" => "跟随窗格目录",
        "Git decorations" => "Git 状态标记",
        "Git footer" => "底部 Git 状态",
        "View as" => "列表视图",
        "Change folder…" => "切换目录…",
        "on" => "开启",
        "off" => "关闭",
        "shown" => "显示",
        "hidden" => "隐藏",
        "material" => "图标字体",
        "emoji" => "表情符号",
        "vscode" => "VS Code",
        "light" => "浅色",
        "terminal" => "跟随终端",
        "tab" => "新标签页",
        "pane" => "分栏窗格",
        "replace" => "替换工作区",
        "not set" => "未设置",
        "on (host n/a)" => "开启（宿主不支持）",
        "off (host n/a)" => "关闭（宿主不支持）",
        "move" => "移动",
        "fold" => "展开/折叠",
        "toggle" => "切换",
        "refresh" => "刷新",
        "dotfiles" => "隐藏文件",
        "folder" => "切换目录",
        "files" => "文件",
        "text" => "全文搜索",
        "menu" => "菜单",
        "settings" => "设置",
        "hide" => "隐藏",
        "quit" => "退出",
        "search" => "搜索",
        "git" => "Git",
        "stage" => "暂存",
        "list" => "列表",
        "tree" => "树形",
        "all" => "全部暂存",
        "none" => "取消暂存",
        "msg" => "提交说明",
        "suggest" => "生成说明",
        "diff" => "差异",
        "sync" => "同步",
        "Close pane" => "关闭窗格",
        "close" => "关闭",
        _ => value,
    }
}

pub fn row(label: &str, value: &str) -> (String, String) {
    let translated = match label {
        "Sidebar width" => value.replace(" cols", " 列"),
        "Custom editor..." if value == "not set" => text(value).into(),
        "Custom editor..." | "Change folder…" | "Open in VS Code" | "HerdrWork version"
        | "Sidebar version" => value.into(),
        _ => text(value).into(),
    };
    (text(label).into(), translated)
}

/// Fit two columns using terminal cell width, including Chinese and long paths.
pub fn row_text(label: &str, value: &str, width: usize) -> String {
    if width < 3 {
        return truncate_to(label.to_string(), width);
    }
    let label = truncate_to(label.to_string(), width - 3);
    let label_width = Span::raw(&label).width();
    let value = truncate_to(value.to_string(), width - label_width - 3);
    let padding = width - label_width - Span::raw(&value).width() - 2;
    format!(" {label}{}{value} ", " ".repeat(padding))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_settings_fit_narrow_columns_without_overflow() {
        for width in [0, 1, 2, 12, 26, 38, 80] {
            for (label, value) in [
                ("侧栏宽度", "28 列"),
                ("切换目录…", "项目/很长的中文目录🗂️"),
            ] {
                let line = row_text(label, value, width);
                assert!(Span::raw(line).width() <= width);
            }
        }
    }
}
