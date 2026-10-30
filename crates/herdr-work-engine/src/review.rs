use crate::{HerdrWork, storage};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use herdr_work_core::{
    BatchId, Comment, CommentAnchor, Delivery, Execution, Review, ReviewBatch, ReviewId, Side,
};
use serde::{Deserialize, Serialize};
use std::{fs::OpenOptions, path::PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendOutcome {
    pub batch: BatchId,
    pub state: Delivery,
    pub focus_warning: bool,
}
impl HerdrWork {
    fn review_path(&self, id: &ReviewId) -> PathBuf {
        self.root()
            .join("reviews")
            .join(format!("{}.json", storage::key(&id.0)))
    }
    fn review_lock(&self, id: &ReviewId) -> Result<std::fs::File> {
        std::fs::create_dir_all(self.root().join("reviews"))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.review_path(id).with_extension("lock"))?;
        lock.lock_exclusive()?;
        Ok(lock)
    }
    pub fn open_review(
        &self,
        execution: &Execution,
        scope: &str,
        base: Option<String>,
    ) -> Result<Review> {
        Self::validate_execution(execution)?;
        let id = ReviewId(storage::key(&serde_json::to_string(&(
            &execution.id,
            &execution.worktree_identity,
            scope,
            &base,
        ))?));
        let _lock = self.review_lock(&id)?;
        if self.review_path(&id).exists() {
            let mut review = self.review(&id)?;
            ensure!(
                review.execution.worktree == execution.worktree,
                "Review worktree changed"
            );
            review.execution = execution.clone();
            storage::save(&self.review_path(&id), &review)?;
            return Ok(review);
        }
        let review = Review {
            schema_version: 1,
            id,
            execution: execution.clone(),
            base,
            scope: scope.into(),
            comments: vec![],
            batches: vec![],
            next_comment: 1,
        };
        storage::save(&self.review_path(&review.id), &review)?;
        Ok(review)
    }
    pub fn review(&self, id: &ReviewId) -> Result<Review> {
        let review: Review = storage::load(&self.review_path(id))?;
        ensure!(
            review.schema_version == 1 && &review.id == id,
            "Unsupported review state"
        );
        Ok(review)
    }
    pub fn add_comment(
        &self,
        id: &ReviewId,
        file: &str,
        side: Side,
        start: u32,
        end: u32,
        text: &str,
    ) -> Result<Review> {
        self.add_comment_checked(id, file, side, start, end, text, None)
    }
    pub fn add_comment_at(
        &self,
        id: &ReviewId,
        anchor: &CommentAnchor,
        text: &str,
    ) -> Result<Review> {
        self.add_comment_checked(
            id,
            &anchor.file,
            anchor.side,
            anchor.start,
            anchor.end,
            text,
            Some(&anchor.fingerprint),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn add_comment_checked(
        &self,
        id: &ReviewId,
        file: &str,
        side: Side,
        start: u32,
        end: u32,
        text: &str,
        expected: Option<&str>,
    ) -> Result<Review> {
        ensure!(
            start > 0 && end >= start && end - start < 1000 && !text.trim().is_empty(),
            "Invalid comment range or empty text"
        );
        let _lock = self.review_lock(id)?;
        let mut review = self.review(id)?;
        Self::validate_execution(&review.execution)?;
        let diff = self.diff(
            &review.execution.worktree,
            file,
            &review.scope,
            review.base.as_deref(),
        )?;
        ensure!(
            expected.is_none_or(|fingerprint| fingerprint == diff.fingerprint),
            "Displayed code changed while composing; refresh and re-anchor the comment"
        );
        let rows: Vec<_> = diff
            .rows
            .iter()
            .filter(|r| {
                let n = if side == Side::Old { r.old } else { r.new };
                n.is_some_and(|n| n >= start && n <= end)
            })
            .collect();
        ensure!(
            rows.len() == (end - start + 1) as usize,
            "Selected lines are not present on this diff side"
        );
        let snippet = rows
            .iter()
            .map(|r| format!("{}{}", r.marker, r.text))
            .collect::<Vec<_>>()
            .join("\n");
        let comment = Comment {
            id: review.next_comment,
            version: 1,
            file: file.into(),
            side,
            start,
            end,
            snippet,
            text: text.into(),
            fingerprint: diff.fingerprint,
        };
        review.next_comment += 1;
        review.comments.push(comment);
        storage::save(&self.review_path(id), &review)?;
        Ok(review)
    }
    pub fn edit_comment(&self, id: &ReviewId, comment_id: u64, text: &str) -> Result<Review> {
        ensure!(!text.trim().is_empty(), "Comment is empty");
        let _lock = self.review_lock(id)?;
        let mut review = self.review(id)?;
        let comment = review
            .comments
            .iter_mut()
            .find(|c| c.id == comment_id)
            .context("Comment not found")?;
        comment.version += 1;
        comment.text = text.into();
        storage::save(&self.review_path(id), &review)?;
        Ok(review)
    }
    pub fn stale_comments(&self, review: &Review) -> Result<Vec<u64>> {
        review
            .comments
            .iter()
            .filter_map(|c| {
                match self.diff(
                    &review.execution.worktree,
                    &c.file,
                    &review.scope,
                    review.base.as_deref(),
                ) {
                    Ok(diff) if diff.fingerprint == c.fingerprint => None,
                    Ok(_) => Some(Ok(c.id)),
                    Err(e) => Some(Err(e)),
                }
            })
            .collect()
    }
    pub fn prepare_send(&self, id: &ReviewId) -> Result<ReviewBatch> {
        let _lock = self.review_lock(id)?;
        let mut review = self.review(id)?;
        Self::validate_execution(&review.execution)?;
        ensure!(!review.comments.is_empty(), "No comments to send");
        ensure!(
            !review.batches.iter().any(|b| matches!(
                b.state,
                Delivery::Prepared | Delivery::Sending | Delivery::Unknown
            )),
            "Pending or unknown delivery needs explicit reconciliation"
        );
        ensure!(
            self.stale_comments(&review)?.is_empty(),
            "Code changed; stale comments require a new review"
        );
        let target = review
            .execution
            .target
            .clone()
            .context("Review execution is not bound")?;
        let mut comments = review.comments.iter().collect::<Vec<_>>();
        comments.sort_by(|a, b| a.file.cmp(&b.file).then(a.start.cmp(&b.start)));
        let text = comments
            .into_iter()
            .map(|comment| {
                let range = if comment.start == comment.end {
                    format!("{}:{}", comment.file, comment.start)
                } else {
                    format!("{}:{}-{}", comment.file, comment.start, comment.end)
                };
                let removed = if comment.side == Side::Old {
                    " (removed)"
                } else {
                    ""
                };
                let text = comment
                    .text
                    .replace('\r', "")
                    .lines()
                    .map(str::trim_end)
                    .filter(|line| !line.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{range}{removed}\n{}\n{text}", comment.snippet)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let batch = ReviewBatch {
            id: BatchId(format!(
                "{}-{}",
                review.id,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
            )),
            target,
            comments: review.comments.iter().map(|c| (c.id, c.version)).collect(),
            text,
            state: Delivery::Prepared,
        };
        review.batches.push(batch.clone());
        storage::save(&self.review_path(id), &review)?;
        Ok(batch)
    }
    /// Apply a receipt to its original batch, preserving comments edited after preparation.
    pub fn record_delivery(
        &self,
        id: &ReviewId,
        batch: &BatchId,
        state: Delivery,
    ) -> Result<Review> {
        let _lock = self.review_lock(id)?;
        let mut review = self.review(id)?;
        let record = review
            .batches
            .iter_mut()
            .find(|b| &b.id == batch)
            .context("Batch not found")?;
        ensure!(
            record.state != Delivery::Delivered || state == Delivery::Delivered,
            "Delivered batch cannot be retried"
        );
        record.state = state;
        if state == Delivery::Delivered {
            let sent = record.comments.clone();
            review
                .comments
                .retain(|c| !sent.contains(&(c.id, c.version)));
        }
        storage::save(&self.review_path(id), &review)?;
        Ok(review)
    }
    pub fn send_review(&self, id: &ReviewId) -> Result<SendOutcome> {
        let review = self.review(id)?;
        let target = self.validate_target(&review.execution)?;
        ensure!(
            std::env::var_os("HERDR_SOCKET_PATH").as_deref() == Some(target.socket.as_os_str()),
            "Active endpoint differs from bound endpoint"
        );
        let batch = self.prepare_send(id)?;
        ensure!(
            batch.target.socket == target.socket
                && batch.target.session == target.session
                && batch.target.terminal == target.terminal,
            "Binding changed during preparation; reconcile the prepared batch before sending"
        );
        self.record_delivery(id, &batch.id, Delivery::Sending)?;
        let state = match crate::transport::send(&target.socket, &target.pane, &batch.text) {
            Ok(()) => Delivery::Delivered,
            Err(crate::transport::SendFailure::Unknown) => Delivery::Unknown,
            Err(crate::transport::SendFailure::Rejected) => Delivery::Failed,
        };
        self.record_delivery(id, &batch.id, state)?;
        let focus_warning = state == Delivery::Delivered && self.focus(&review.execution).is_err();
        Ok(SendOutcome {
            batch: batch.id,
            state,
            focus_warning,
        })
    }
    pub fn resolve_batch(&self, id: &ReviewId, batch: &BatchId, delivered: bool) -> Result<Review> {
        let review = self.review(id)?;
        let b = review
            .batches
            .iter()
            .find(|b| &b.id == batch)
            .context("Batch not found")?;
        ensure!(
            matches!(b.state, Delivery::Unknown | Delivery::Prepared),
            "Only unresolved batches may be manually reconciled"
        );
        self.record_delivery(
            id,
            batch,
            if delivered {
                Delivery::Delivered
            } else {
                Delivery::Failed
            },
        )
    }
    pub(crate) fn recover_reviews(&self) -> Result<()> {
        let dir = self.root().join("reviews");
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let mut review: Review = storage::load(&path)?;
            let mut changed = false;
            for batch in &mut review.batches {
                if batch.state == Delivery::Sending {
                    batch.state = Delivery::Unknown;
                    changed = true;
                }
            }
            if changed {
                storage::save(&path, &review)?;
            }
        }
        Ok(())
    }
}
