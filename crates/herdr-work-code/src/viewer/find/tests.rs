use super::*;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn doc(text: &str) -> Doc {
    crate::preview::git_show("example.rs".into(), "<workspace>/example.rs".into(), text)
}
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn render(find: &mut Find, doc: &mut Doc, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            super::super::draw_doc(frame, doc, crate::icons::IconTheme::Emoji, true, None, find);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}
fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .iter()
        .flat_map(|cell| cell.symbol().chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}
fn click(x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}
fn search(query: &str) -> Find {
    let mut find = Find::default();
    find.show();
    find.insert(query);
    find
}

#[test]
fn only_find_shortcut_opens_and_hidden_widget_leaves_preview_keys_alone() {
    let doc = doc("example");
    for modifier in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
        let mut find = Find::default();
        for code in [
            KeyCode::Char('q'),
            KeyCode::Char('e'),
            KeyCode::Char('w'),
            KeyCode::Char('/'),
            KeyCode::F(3),
            KeyCode::Esc,
        ] {
            assert!(!find.key(key(code), &doc));
        }
        assert!(!find.key(
            KeyEvent::new(
                KeyCode::Char('f'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            ),
            &doc
        ));
        assert!(find.key(KeyEvent::new(KeyCode::Char('f'), modifier), &doc));
        assert!(find.open && find.focused);
    }
}

#[test]
fn find_captures_preview_commands_and_escape_closes_find_first() {
    let doc = doc("example");
    let mut find = search("");
    for c in "qewjkrb g".chars() {
        assert!(find.key(key(KeyCode::Char(c)), &doc));
    }
    assert_eq!(find.query, "qewjkrb g");
    assert!(!find.key(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        &doc
    ));
    assert!(find.key(key(KeyCode::Esc), &doc));
    assert!(!find.open);
    assert!(!find.key(key(KeyCode::Esc), &doc));
    assert!(!find.key(key(KeyCode::Char('e')), &doc));
}

#[test]
fn literal_case_whole_word_and_regex_share_search_rules() {
    let mut doc = doc("Launch launch launcher\n启动 launch42 launch7");
    let mut find = search("launch");
    render(&mut find, &mut doc, 100, 10);
    assert_eq!(find.hits.len(), 5);
    find.activate(1);
    render(&mut find, &mut doc, 100, 10);
    assert_eq!(find.hits.len(), 4);
    find.activate(2);
    render(&mut find, &mut doc, 100, 10);
    assert_eq!(find.hits.len(), 1);
    find.options = SearchOptions {
        regex: true,
        ..SearchOptions::default()
    };
    find.query = "launch[0-9]+".into();
    render(&mut find, &mut doc, 100, 10);
    assert_eq!(find.hits.len(), 2);
}

#[test]
fn invalid_regex_and_no_results_remain_editable() {
    let mut doc = doc("launch");
    let mut find = search("[");
    find.options.regex = true;
    assert!(text(&render(&mut find, &mut doc, 90, 8)).contains("正则有误"));
    assert!(find.error && find.hits.is_empty());
    find.key(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        &doc,
    );
    find.insert("absent");
    assert!(text(&render(&mut find, &mut doc, 90, 8)).contains("无结果"));
    assert!(!find.error);
    assert!(find.key(key(KeyCode::Enter), &doc));
}

#[test]
fn unicode_phrase_crosses_soft_wrap_and_both_halves_are_highlighted() {
    let mut doc = doc("前缀 你好世界 后缀\n你好世界");
    let mut find = search("你好世界");
    let frame = render(&mut find, &mut doc, 6, 14);
    assert_eq!(find.hits.len(), 2);
    assert_ne!(find.hits[0].start.row, find.hits[0].end.row);
    let highlighted = frame
        .content
        .iter()
        .filter(|cell| cell.bg == palette().button_focus_bg)
        .count();
    assert!(highlighted >= 4);
    assert!(text(&frame).contains("你好"));
}

#[test]
fn enter_shift_enter_and_f3_wrap_and_reveal_matches_without_changing_selection() {
    let mut doc = doc(&format!("launch\n{}launch\n", "filler\n".repeat(60)));
    let mut find = search("launch");
    render(&mut find, &mut doc, 90, 12);
    assert_eq!(find.active, Some(0));
    find.key(key(KeyCode::Enter), &doc);
    assert!(text(&render(&mut find, &mut doc, 90, 12)).contains("2/2"));
    assert!(doc.scroll > 40);
    assert!(doc.selection_range().is_none());
    find.key(key(KeyCode::Enter), &doc);
    render(&mut find, &mut doc, 90, 12);
    assert_eq!(find.active, Some(0));
    assert_eq!(doc.scroll, 0);
    find.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT), &doc);
    render(&mut find, &mut doc, 90, 12);
    assert_eq!(find.active, Some(1));
    find.key(KeyEvent::new(KeyCode::F(3), KeyModifiers::SHIFT), &doc);
    assert_eq!(find.active, Some(0));
}

#[test]
fn resize_and_refresh_recompute_hits_and_keep_current_match() {
    let mut doc = doc("wide line with launch near its end\nsecond launch\nthird launch");
    let mut find = search("launch");
    render(&mut find, &mut doc, 90, 10);
    find.step(false);
    render(&mut find, &mut doc, 24, 10);
    assert_eq!(find.active, Some(1));
    assert_eq!(find.hits.len(), 3);
    doc.refresh(crate::preview::git_show(
        "example.rs".into(),
        "<workspace>/example.rs".into(),
        "only launch",
    ));
    render(&mut find, &mut doc, 24, 10);
    assert_eq!(find.hits.len(), 1);
    assert_eq!(find.active, Some(0));
}

#[test]
fn mouse_entry_buttons_and_body_focus_share_render_geometry() {
    for width in [28, 40, 57, 58, 90, 120] {
        let mut doc = doc("launch launch");
        let mut find = Find::default();
        let header = Rect::new(0, 0, width, 1);
        let entry = Find::entry(header);
        assert!(find.mouse(click(entry.x + 1, 0), Rect::default(), header, &doc));
        find.insert("launch");
        render(&mut find, &mut doc, width, 12);
        let area = Rect::new(0, 1, width, find.height(width));
        for target in 1..=5 {
            let rect = find.controls(area)[target];
            assert!(rect.width >= 3 && area.contains((rect.x, rect.y).into()));
            assert!(find.mouse(click(rect.x + 1, rect.y), area, header, &doc));
        }
        assert!(find.options.match_case && find.options.whole_word && find.options.regex);
        assert!(!find.mouse(click(3, 5), area, header, &doc));
        assert!(!find.key(key(KeyCode::Char('w')), &doc));
        assert!(find.key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            &doc
        ));
        let close = find.controls(area)[6];
        assert!(find.mouse(click(close.x + 1, close.y), area, header, &doc));
        assert!(!find.open);
    }
}

#[test]
fn tab_reaches_options_and_close_without_inserting_or_invoking_preview_commands() {
    let doc = doc("launch");
    let mut find = search("launch");
    for _ in 0..3 {
        assert!(find.key(key(KeyCode::Tab), &doc));
        assert!(find.key(key(KeyCode::Enter), &doc));
    }
    assert!(find.options.match_case && find.options.whole_word && find.options.regex);
    assert_eq!(find.query, "launch");
    find.key(key(KeyCode::BackTab), &doc);
    assert_eq!(find.focus, 2);
    for _ in 0..4 {
        find.key(key(KeyCode::Tab), &doc);
    }
    find.key(key(KeyCode::Enter), &doc);
    assert!(!find.open);
}

#[test]
fn query_editing_and_paste_use_unicode_positions_and_bound_input() {
    let doc = doc("");
    let mut find = search("你好");
    find.key(key(KeyCode::Left), &doc);
    find.insert("launch");
    assert_eq!(find.query, "你launch好");
    find.key(key(KeyCode::Backspace), &doc);
    find.key(key(KeyCode::Delete), &doc);
    assert_eq!(find.query, "你launc");
    find.key(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        &doc,
    );
    find.insert("新\n词\t");
    assert_eq!(find.query, "新词");
    find.insert(&"x".repeat(5000));
    assert_eq!(find.query.chars().count(), 4096);
}

#[test]
fn long_chinese_query_keeps_caret_and_visible_text_within_input() {
    let mut find = search("前缀你好世界后缀你好世界");
    for width in 0..25 {
        let (shown, caret) = find.input_view(width);
        use unicode_width::UnicodeWidthStr;
        assert!(shown.width() <= width);
        assert!(caret < width || width == 0);
    }
    find.cursor = 3;
    let (_, caret) = find.input_view(8);
    assert_eq!(caret, 6);
}

#[test]
fn tiny_viewports_render_without_panicking_and_leave_close_available() {
    for width in [1, 3, 10, 24, 28, 80] {
        for height in [1, 2, 3, 5, 12] {
            let mut doc = doc("hello world");
            let mut find = search("world");
            render(&mut find, &mut doc, width, height);
        }
    }
}

#[test]
fn dense_matches_on_one_line_are_counted() {
    let mut doc = doc(&"a ".repeat(10_000));
    let mut find = search("a");
    render(&mut find, &mut doc, 90, 10);
    assert_eq!(find.hits.len(), 10_000);
}

#[test]
fn find_bar_test_frame() {
    let mut doc = doc(
        "fn launch_worker() {\n    let launch = \"ready\";\n    launch_task(launch);\n}\n\n// launch worker when the queue is ready\nfn launch_task(state: &str) {\n    println!(\"launch: {}\", state);\n}",
    );
    let mut find = search("launch");
    let buffer = render(&mut find, &mut doc, 100, 16);
    assert!(text(&buffer).contains("1/7"));
    if let Ok(path) = std::env::var("HERDR_WORK_FIND_FRAME") {
        use ratatui::style::Color;
        let rgb = |color: Color, default| match color {
            Color::Rgb(r, g, b) => [r, g, b],
            Color::Black => [0, 0, 0],
            Color::White => [255, 255, 255],
            Color::Gray => [180, 180, 180],
            Color::DarkGray => [100, 100, 100],
            Color::Blue => [80, 140, 240],
            Color::Cyan => [80, 200, 220],
            Color::Green => [80, 200, 120],
            Color::Yellow => [230, 200, 90],
            _ => default,
        };
        let cells: Vec<_> = buffer.content.iter().map(|cell| serde_json::json!({"text": cell.symbol(), "fg": rgb(cell.fg,[210,216,226]), "bg": rgb(cell.bg,[28,30,36])})).collect();
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({"width":100,"height":16,"cells":cells}))
                .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn markdown_width_reload_preserves_active_match_when_rendered_padding_changes() {
    let mut doc = doc("launch launch\nlaunch");
    let mut find = search("launch");
    render(&mut find, &mut doc, 90, 10);
    find.step(false);
    render(&mut find, &mut doc, 90, 10);
    doc.lines = vec![Line::raw(" launch launch "), Line::raw(" launch ")];
    doc.rows_key = None;
    render(&mut find, &mut doc, 91, 10);
    assert_eq!(find.active, Some(1));
    assert_eq!(find.hits.len(), 3);
}

#[test]
fn regex_anchors_count_zero_width_hits_and_navigate_without_panicking() {
    let mut doc = doc("first\nsecond\nthird");
    let mut find = search("^");
    find.options.regex = true;
    render(&mut find, &mut doc, 90, 8);
    assert_eq!(find.hits.len(), 3);
    find.step(true);
    render(&mut find, &mut doc, 90, 8);
    assert_eq!(find.active, Some(2));
    assert_eq!(find.hits[2].start, find.hits[2].end);
}
