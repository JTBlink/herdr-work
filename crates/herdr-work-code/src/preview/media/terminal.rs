//! Herdr terminates the graphics protocol itself. Outer-terminal environment
//! hints (notably WEZTERM_EXECUTABLE) must not blacklist its Kitty capability.
use ratatui_image::picker::Picker;

pub(super) fn detect() -> Picker {
    #[cfg(unix)]
    if std::env::var("TERM_PROGRAM").as_deref() == Ok("herdr")
        || std::env::var("HERDR_ENV").as_deref() == Ok("1")
    {
        return query_herdr().unwrap_or_else(|_| Picker::halfblocks());
    }
    Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
}

#[cfg(unix)]
fn query_herdr() -> std::io::Result<Picker> {
    use ratatui_image::picker::cap_parser::{Parser, QueryStdioOptions, Response};
    use std::{
        io::{IsTerminal, Read, Write},
        os::fd::AsRawFd,
        time::{Duration, Instant},
    };

    let mut input = std::io::stdin().lock();
    if !input.is_terminal() {
        return Ok(Picker::halfblocks());
    }
    let mut output = std::io::stdout().lock();
    output.write_all(Parser::query(false, QueryStdioOptions::default()).as_bytes())?;
    output.flush()?;
    drop(output);
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut parser = Parser::new();
    let mut responses = Vec::new();
    while Instant::now() < deadline {
        let mut fd = libc::pollfd {
            fd: input.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as i32;
        // SAFETY: fd points to one initialized pollfd, and stdin remains open.
        let ready = unsafe { libc::poll(&mut fd, 1, remaining) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 || fd.revents & libc::POLLIN == 0 {
            break;
        }
        let mut bytes = [0; 256];
        let count = input.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        for byte in &bytes[..count] {
            responses.extend(parser.push(char::from(*byte)));
        }
        if responses.contains(&Response::Status) {
            break;
        }
    }
    Ok(herdr_picker(&responses))
}

#[cfg(any(unix, test))]
fn herdr_picker(responses: &[ratatui_image::picker::cap_parser::Response]) -> Picker {
    use ratatui_image::{
        FontSize,
        picker::{ProtocolType, cap_parser::Response},
    };
    let size = responses.iter().find_map(|response| match response {
        Response::CellSize(Some((width, height))) if *width > 0 && *height > 0 => {
            Some(FontSize::new(*width, *height))
        }
        _ => None,
    });
    if responses.contains(&Response::Kitty)
        && let Some(size) = size
    {
        // The constructor is needed to supply the queried cell size without
        // re-querying stdio or modifying the process environment.
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(size);
        picker.set_protocol_type(ProtocolType::Kitty);
        picker
    } else {
        Picker::halfblocks()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_image::picker::{
        ProtocolType,
        cap_parser::{Parser, Response},
    };

    #[test]
    fn herdr_capability_reply_selects_kitty_with_queried_cell_size() {
        let mut parser = Parser::new();
        let responses: Vec<_> = "\x1b_Gi=31;OK\x1b\\\x1b[6;20;10t\x1b[0n"
            .chars()
            .flat_map(|ch| parser.push(ch))
            .collect();
        let picker = herdr_picker(&responses);
        assert_eq!(picker.protocol_type(), ProtocolType::Kitty);
        assert_eq!(
            (picker.font_size().width, picker.font_size().height),
            (10, 20)
        );
    }

    #[test]
    fn disabled_graphics_or_missing_cell_size_keeps_fallback() {
        for responses in [
            vec![Response::CellSize(Some((10, 20))), Response::Status],
            vec![Response::Kitty, Response::Status],
            vec![Response::Kitty, Response::CellSize(Some((0, 0)))],
        ] {
            assert_eq!(
                herdr_picker(&responses).protocol_type(),
                ProtocolType::Halfblocks
            );
        }
    }
}
