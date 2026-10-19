//! Sidebar commit input editing, shared by source and product interfaces.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn edit_message(message: &mut Vec<char>, cursor: &mut usize, key: KeyEvent) {
    *cursor = (*cursor).min(message.len());
    match key.code {
        KeyCode::Backspace if *cursor > 0 => {
            *cursor -= 1;
            message.remove(*cursor);
        }
        KeyCode::Delete if *cursor < message.len() => {
            message.remove(*cursor);
        }
        KeyCode::Left => *cursor = cursor.saturating_sub(1),
        KeyCode::Right => *cursor = (*cursor + 1).min(message.len()),
        KeyCode::Home => *cursor = 0,
        KeyCode::End => *cursor = message.len(),
        KeyCode::Char('u')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            message.clear();
            *cursor = 0;
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT) =>
        {
            message.insert(*cursor, c);
            *cursor += 1;
        }
        _ => {}
    }
}
