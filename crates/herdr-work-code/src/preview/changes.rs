//! Navigation uses parsed change blocks, never colors or wrapped screen rows.
use super::{Doc, PreviewSelection};

pub(crate) struct ChangeNavigation {
    starts: Vec<usize>,
    pub(crate) active: Option<usize>,
}

impl ChangeNavigation {
    pub(crate) fn new(starts: Vec<usize>) -> Self {
        Self {
            starts,
            active: None,
        }
    }
}

impl Doc {
    pub(crate) fn change_count(&self) -> Option<(usize, usize)> {
        self.changes.as_ref().map(|changes| {
            (
                changes.active.map_or(0, |index| index + 1),
                changes.starts.len(),
            )
        })
    }

    /// Wrap at either end. Keep an explicit cursor: several blocks near EOF
    /// may share the same clamped viewport, but must remain separate targets.
    pub(crate) fn step_change(&mut self, reverse: bool) {
        let top = self.top_src();
        let Some(changes) = &mut self.changes else {
            return;
        };
        let count = changes.starts.len();
        if count == 0 {
            return;
        }
        let index = match changes.active {
            Some(at) if reverse => (at + count - 1) % count,
            Some(at) => (at + 1) % count,
            None if reverse => changes
                .starts
                .partition_point(|src| *src < top)
                .checked_sub(1)
                .unwrap_or(count - 1),
            None => changes
                .starts
                .iter()
                .position(|src| *src >= top)
                .unwrap_or(0),
        };
        changes.active = Some(index);
        self.pending_src = Some(changes.starts[index]);
        self.selection = PreviewSelection::default();
    }
}
