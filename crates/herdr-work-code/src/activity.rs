//! Pure activity values shared with the retained source launch adapter.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitialView {
    Explorer,
    Search,
    Git,
    QuickOpen,
}
impl InitialView {
    pub fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "explorer" => Some(Self::Explorer),
            "search" => Some(Self::Search),
            "source-control" => Some(Self::Git),
            "quick-open" => Some(Self::QuickOpen),
            _ => None,
        }
    }
    pub fn env_value(self) -> &'static str {
        match self {
            Self::Explorer => "explorer",
            Self::Search => "search",
            Self::Git => "source-control",
            Self::QuickOpen => "quick-open",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Explorer => "f9",
            Self::Search => "f10",
            Self::Git => "f11",
            Self::QuickOpen => "f12",
        }
    }
}
