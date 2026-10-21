//! Adapt product snapshots to Sidebar's original row widgets and geometry.
use crate::{
    scm::{ScmRow, ScmTarget},
    state::*,
};
use herdr_work_code::{
    git::{FileEntry, Status},
    scm_widgets as source,
};
use ratatui::layout::{Position, Rect};

pub(crate) struct ScmLayout {
    pub rows: Vec<ScmRow>,
    heights: Vec<u16>,
    pub visible: std::ops::Range<usize>,
}
impl ScmLayout {
    pub fn hit(&self, body: Rect, point: Position) -> Option<(usize, u16)> {
        if !body.contains(point) {
            return None;
        }
        source::row_hit_with_heights(
            source::BodyGeom {
                top: body.y,
                height: body.height,
                offset: self.visible.start,
            },
            point.y,
            self.visible
                .clone()
                .map(|index| (index, self.heights[index])),
        )
    }
    pub fn rect(&self, body: Rect, index: usize) -> Option<Rect> {
        if !self.visible.contains(&index) {
            return None;
        }
        let y = body
            .y
            .saturating_add(self.heights[self.visible.start..index].iter().sum());
        Some(Rect::new(
            body.x,
            y,
            body.width,
            self.heights[index].min(body.bottom().saturating_sub(y)),
        ))
    }
}
impl App {
    pub(crate) fn scm_layout(&self, body: Rect) -> ScmLayout {
        let rows = self.scm_rows();
        let heights = rows
            .iter()
            .map(|row| match row.3 {
                ScmTarget::Message(index) => {
                    let draft = self
                        .code
                        .as_ref()
                        .and_then(|code| code.repositories.get(index))
                        .and_then(|repo| self.git.drafts.get(&repo.root));
                    source::message_box_height(
                        draft.map_or(&[], |d| d.message.as_slice()),
                        draft.map_or(0, |d| d.cursor),
                        body.width,
                    )
                }
                ScmTarget::Commit(_) => 1,
                _ => 1,
            })
            .collect::<Vec<_>>();
        let visible = source::list_window(&heights, 0, Some(self.scm_cursor), body.height);
        ScmLayout {
            rows,
            heights,
            visible,
        }
    }
    pub(crate) fn sidebar_repo_status(&self, index: usize) -> Status {
        let Some((code, repo)) = self
            .code
            .as_ref()
            .and_then(|code| code.repositories.get(index).map(|repo| (code, repo)))
        else {
            return Status::default();
        };
        let mut status = Status {
            branch: repo.branch.clone(),
            ahead: repo.ahead,
            behind: repo.behind,
            has_upstream: repo.has_upstream,
            ..Default::default()
        };
        for change in code
            .changes
            .iter()
            .filter(|change| change.root == repo.root)
        {
            let entry = FileEntry {
                path: change.path.clone(),
                orig: change.original.clone(),
                letter: change.status,
            };
            if change.staged {
                status.staged.push(entry);
            } else {
                status.unstaged.push(entry);
            }
        }
        status
    }
}
