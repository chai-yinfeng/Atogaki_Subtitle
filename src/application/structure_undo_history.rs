//! Session-only history. A service must serialize mutation+record and undo under
//! the same gate; receipts are moved here, never cloned into a second history.
use crate::infrastructure::local_db::{
    LocalDatabase, LocalStructureEdit, LocalSubtitleSegmentRecord,
};
use anyhow::{Result, anyhow, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const STRUCTURE_UNDO_BUDGET_BYTES: usize = 96 * 1024 * 1024;
const PER_JOB_LIMIT: usize = 10;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StructureUndoDescriptor {
    pub token: String,
    pub label: String,
    pub expected_revision: i64,
}
#[derive(Debug, Serialize)]
pub struct StructureHistoryRecord {
    pub latest: Option<StructureUndoDescriptor>,
    pub discarded_entries: usize,
    pub oversized: bool,
}
#[derive(Debug, Serialize)]
pub struct StructureHistoryUndo {
    pub segments: Vec<LocalSubtitleSegmentRecord>,
    pub workspace_revision: i64,
    pub translation_context_stale_ids: Vec<String>,
    pub latest: Option<StructureUndoDescriptor>,
    pub discarded_entries: usize,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct StructureHistoryStats {
    pub jobs: usize,
    pub entries: usize,
    /// Receipt capacities plus entry/label storage. Not process RSS or allocator overhead.
    pub retained_bytes: usize,
    pub budget_bytes: usize,
}
struct Entry {
    edit: LocalStructureEdit,
    label: String,
    order: u64,
}
impl Entry {
    fn bytes(&self) -> usize {
        self.edit.retained_bytes() + self.label.capacity() + std::mem::size_of::<Self>()
            - std::mem::size_of::<LocalStructureEdit>()
    }
    fn descriptor(&self) -> StructureUndoDescriptor {
        StructureUndoDescriptor {
            token: self.edit.token().into(),
            label: self.label.clone(),
            expected_revision: self.edit.workspace_revision(),
        }
    }
}
fn stored_bytes(entries: &VecDeque<Entry>) -> usize {
    entries.iter().map(Entry::bytes).sum::<usize>()
        + (entries.capacity() - entries.len()) * std::mem::size_of::<Entry>()
}
/// Bounded across all jobs; oldest job heads are indexed without retaining stale queue nodes.
pub struct StructureUndoHistory {
    jobs: BTreeMap<String, VecDeque<Entry>>,
    heads: BTreeSet<(u64, String)>,
    retained_bytes: usize,
    budget: usize,
    order: u64,
}
impl Default for StructureUndoHistory {
    fn default() -> Self {
        Self::with_budget(STRUCTURE_UNDO_BUDGET_BYTES)
    }
}
impl StructureUndoHistory {
    pub fn with_budget(budget: usize) -> Self {
        Self {
            jobs: BTreeMap::new(),
            heads: BTreeSet::new(),
            retained_bytes: 0,
            budget,
            order: 0,
        }
    }
    fn take(&mut self, job: &str) -> VecDeque<Entry> {
        let entries = self.jobs.remove(job).unwrap_or_default();
        if let Some(first) = entries.front() {
            self.heads.remove(&(first.order, job.into()));
        }
        self.retained_bytes -= stored_bytes(&entries);
        entries
    }
    fn put(&mut self, job: String, mut entries: VecDeque<Entry>) {
        if let Some(first) = entries.front() {
            self.heads.insert((first.order, job.clone()));
            entries.shrink_to_fit();
            self.retained_bytes += stored_bytes(&entries);
            self.jobs.insert(job, entries);
        }
    }
    fn trim_budget(&mut self) -> usize {
        let mut discarded = 0;
        while self.retained_bytes > self.budget {
            let Some((_, job)) = self.heads.first().cloned() else {
                break;
            };
            let mut entries = self.take(&job);
            entries.pop_front();
            discarded += 1;
            self.put(job, entries);
        }
        discarded
    }
    pub fn latest(&self, job: &str) -> Option<StructureUndoDescriptor> {
        self.jobs.get(job)?.back().map(Entry::descriptor)
    }
    pub fn stats(&self) -> StructureHistoryStats {
        StructureHistoryStats {
            jobs: self.jobs.len(),
            entries: self.jobs.values().map(VecDeque::len).sum(),
            retained_bytes: self.retained_bytes,
            budget_bytes: self.budget,
        }
    }
    pub fn forget_job(&mut self, job: &str) -> usize {
        self.take(job).len()
    }
    pub fn clear(&mut self) {
        self.jobs.clear();
        self.heads.clear();
        self.retained_bytes = 0;
    }

    /// Call only with a committed receipt, under the service mutation gate.
    /// Oversized committed edits clear their job's older history and report no token.
    pub fn record(&mut self, edit: LocalStructureEdit, label: String) -> StructureHistoryRecord {
        let job = edit.job_id().to_owned();
        let mut entries = self.take(&job);
        let mut discarded = 0;
        if entries
            .back()
            .is_some_and(|previous| !edit.follows(&previous.edit))
        {
            discarded += entries.len();
            entries.clear();
        }
        self.order = self.order.saturating_add(1);
        let entry = Entry {
            edit,
            label,
            order: self.order,
        };
        if entry.bytes() > self.budget {
            discarded += entries.len();
            return StructureHistoryRecord {
                latest: None,
                discarded_entries: discarded,
                oversized: true,
            };
        }
        entries.push_back(entry);
        while entries.len() > PER_JOB_LIMIT {
            entries.pop_front();
            discarded += 1;
        }
        self.put(job.clone(), entries);
        discarded += self.trim_budget();
        StructureHistoryRecord {
            latest: self.latest(&job),
            discarded_entries: discarded,
            oversized: false,
        }
    }

    pub async fn undo(
        &mut self,
        database: &LocalDatabase,
        job: &str,
        token: &str,
    ) -> Result<StructureHistoryUndo> {
        let top = self
            .jobs
            .get(job)
            .and_then(|entries| entries.back())
            .ok_or_else(|| anyhow!("no structure undo history for this job"))?;
        ensure!(
            top.edit.token() == token,
            "structure undo token is stale or belongs to another job"
        );
        // Failure retains the receipt for an explicit retry; SQLite restores atomically.
        let restored = database.undo_recorded_structure(&top.edit).await?;
        let mut entries = self.take(job);
        let undone = entries
            .pop_back()
            .expect("the checked history is held by this mutable session");
        let mut discarded = 0;
        if let Some(previous) = entries.back_mut() {
            if previous
                .edit
                .rebase_after_undo(&undone.edit, &restored)
                .is_err()
            {
                discarded = entries.len();
                entries.clear();
            }
        }
        self.put(job.into(), entries);
        discarded += self.trim_budget();
        Ok(StructureHistoryUndo {
            segments: restored.segments().to_vec(),
            workspace_revision: restored.workspace_revision(),
            translation_context_stale_ids: restored.translation_context_stale_ids(),
            latest: self.latest(job),
            discarded_entries: discarded,
        })
    }
}

#[cfg(test)]
mod tests;
