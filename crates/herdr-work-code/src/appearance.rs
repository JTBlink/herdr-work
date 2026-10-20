//! Shared visual preferences; no plugin configuration or lifecycle calls.
pub const DEFAULT_SIDEBAR_WIDTH: u16 = 32;
/// Product sidebar default; the reference Sidebar keeps its original width.
pub const PRODUCT_SIDEBAR_WIDTH: u16 = 28;
pub const MIN_SIDEBAR_WIDTH: u16 = 24;
pub const MAX_SIDEBAR_WIDTH: u16 = 80;
pub const SIDEBAR_WIDTH_STEP: u16 = 4;

pub fn clamp_sidebar_width(width: u16) -> u16 {
    width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
}

pub fn step_sidebar_width(width: u16, wider: bool) -> u16 {
    let width = clamp_sidebar_width(width);
    if wider {
        (width + SIDEBAR_WIDTH_STEP).min(MAX_SIDEBAR_WIDTH)
    } else {
        width
            .saturating_sub(SIDEBAR_WIDTH_STEP)
            .max(MIN_SIDEBAR_WIDTH)
    }
}

/// Accent palette for the sidebar. `VsCode` preserves the historical RGB
/// styling, which assumes a DARK terminal background; `Light` is its
/// light-background counterpart; `Terminal` uses ANSI colors so the terminal
/// profile remaps them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorTheme {
    VsCode,
    Light,
    Terminal,
}

impl ColorTheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::VsCode => "vscode",
            Self::Light => "light",
            Self::Terminal => "terminal",
        }
    }

    /// The Settings row cycles through every theme — a rotation, not a
    /// two-way toggle.
    pub fn next(self) -> Self {
        match self {
            Self::VsCode => Self::Light,
            Self::Light => Self::Terminal,
            Self::Terminal => Self::VsCode,
        }
    }

    /// This palette is drawn for a LIGHT terminal background: the preview's
    /// syntax theme, diff tints and icon colors follow it.
    pub fn is_light(self) -> bool {
        self == Self::Light
    }

    pub fn from_state_name(name: &str) -> Option<Self> {
        match name {
            "vscode" => Some(Self::VsCode),
            "light" => Some(Self::Light),
            "terminal" => Some(Self::Terminal),
            _ => None,
        }
    }
}
