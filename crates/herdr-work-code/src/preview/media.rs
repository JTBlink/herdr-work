//! Native terminal graphics; encoding never owns terminal input or stdout.
use image::RgbaImage;
use ratatui::{
    Frame,
    layout::{Rect, Size},
};
use ratatui_image::{
    FilterType, Resize, StatefulImage,
    picker::{Picker, ProtocolType},
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
};
use std::sync::{
    OnceLock,
    mpsc::{self, Receiver},
};

static PICKER: OnceLock<Picker> = OnceLock::new();
mod terminal;

/// Call once from the terminal owner, after init and before reading events.
/// Workers and draw methods must never query stdin.
pub fn initialize_terminal_images() {
    PICKER.get_or_init(terminal::detect);
}

pub(crate) struct NativePreview {
    protocol: ThreadProtocol,
    results: Receiver<Result<ResizeResponse, ratatui_image::errors::Errors>>,
    size: Size,
    failed: bool,
}

impl NativePreview {
    pub(super) fn new(pixels: &RgbaImage) -> Option<Self> {
        Self::with_picker(pixels, PICKER.get()?)
    }

    fn with_picker(pixels: &RgbaImage, picker: &Picker) -> Option<Self> {
        if picker.protocol_type() == ProtocolType::Halfblocks {
            return None;
        }
        let (requests, input) = mpsc::channel::<ResizeRequest>();
        let (output, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = input.recv() {
                if output.send(request.resize_encode()).is_err() {
                    break;
                }
            }
        });
        Some(Self {
            protocol: ThreadProtocol::new(
                requests,
                Some(picker.new_resize_protocol(pixels.clone().into())),
            ),
            results,
            size: Size::default(),
            failed: false,
        })
    }

    pub(super) fn pending(&self) -> bool {
        !self.failed && self.protocol.protocol_type().is_none()
    }

    pub(super) fn draw(&mut self, frame: &mut Frame, body: Rect) -> bool {
        while let Ok(result) = self.results.try_recv() {
            match result {
                Ok(response) => {
                    self.protocol.update_resized_protocol(response);
                }
                Err(_) => self.failed = true,
            }
        }
        if self.failed {
            return false;
        }
        if body.is_empty() {
            return true;
        }
        let resize = Resize::Fit(Some(FilterType::Lanczos3));
        if let Some(size) = self.protocol.size_for(resize.clone(), body.into()) {
            self.size = size;
        }
        // Never expose stale pixels outside the current viewport during resize.
        let width = self.size.width.min(body.width);
        let height = self.size.height.min(body.height);
        let area = Rect::new(
            body.x + (body.width - width) / 2,
            body.y + (body.height - height) / 2,
            width,
            height,
        );
        frame.render_stateful_widget(
            StatefulImage::new().resize(resize),
            area,
            &mut self.protocol,
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use std::time::{Duration, Instant};

    #[test]
    fn unsupported_terminal_keeps_portable_preview() {
        let pixels = RgbaImage::new(2, 2);
        assert!(NativePreview::with_picker(&pixels, &Picker::halfblocks()).is_none());
    }

    #[test]
    fn native_preview_preserves_pixels_and_follows_viewport() {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(ratatui_image::FontSize::new(10, 20));
        picker.set_protocol_type(ProtocolType::Iterm2);
        let pixels = RgbaImage::from_pixel(160, 100, image::Rgba([30, 60, 90, 255]));
        let mut preview = NativePreview::with_picker(&pixels, &picker).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
        for viewport in [Rect::new(3, 2, 30, 15), Rect::new(5, 4, 8, 4)] {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                terminal
                    .draw(|frame| {
                        assert!(preview.draw(frame, viewport));
                    })
                    .unwrap();
                if !preview.pending() {
                    break;
                }
                assert!(Instant::now() < deadline, "image encoding timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(preview.size.width <= viewport.width);
            assert!(preview.size.height <= viewport.height);
            let symbols: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(
                symbols.contains("1337;File="),
                "must render native image data"
            );
            assert!(!symbols.contains('▀'));
        }
        // Fit retains the 160-pixel source in a 30-column viewport, rather
        // than reducing it to one pixel per column.
        let protocol = picker
            .new_protocol(pixels.into(), Size::new(30, 15), Resize::Fit(None))
            .unwrap();
        assert_eq!(protocol.size(), Size::new(16, 5));
    }
}
