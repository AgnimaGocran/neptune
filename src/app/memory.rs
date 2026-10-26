//! The memory each tab's process tree holds, measured off the frame.
//!
//! The session worker owns the shell, so it measures the tree it spawned and
//! answers a bounded request. The frame never waits for a reading: it shows
//! the last one, and asks for a new one every few seconds.
use std::{
    collections::HashMap,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use neptune_model::PaneId;

use crate::runtime::sessions::SessionManager;

/// How often a tab's memory is measured.
const INTERVAL: Duration = Duration::from_secs(3);

/// The last measurement of every tab that was on screen.
pub(super) struct Memory {
    /// The last measurement of each tab, in bytes.
    values: HashMap<PaneId, u64>,
    /// The requests in flight, one per tab.
    pending: HashMap<PaneId, Receiver<Option<u64>>>,
    /// When the next round of measurements starts.
    next: Instant,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    pub(super) fn new() -> Self {
        Self {
            values: HashMap::new(),
            pending: HashMap::new(),
            next: Instant::now(),
        }
    }
    /// The memory of a tab, in bytes, as it was last measured.
    pub(super) fn bytes(&self, pane: PaneId) -> Option<u64> {
        self.values.get(&pane).copied()
    }

    /// Measure the tabs that are on screen. A reading arrives later; the
    /// frame never waits for it.
    pub(super) fn poll(&mut self, sessions: &SessionManager, panes: &[PaneId]) {
        let mut answered = Vec::new();
        for (pane, request) in &self.pending {
            if let Ok(reading) = request.try_recv() {
                answered.push((*pane, reading));
            }
        }
        for (pane, reading) in answered {
            self.pending.remove(&pane);
            match reading {
                Some(bytes) => {
                    self.values.insert(pane, bytes);
                }
                None => {
                    self.values.remove(&pane);
                }
            }
        }
        self.values.retain(|pane, _| panes.contains(pane));
        self.pending.retain(|pane, _| panes.contains(pane));
        if Instant::now() < self.next {
            return;
        }
        self.next = Instant::now() + INTERVAL;
        for pane in panes {
            if self.pending.contains_key(pane) {
                continue;
            }
            if let Some(session) = sessions.get(*pane) {
                self.pending.insert(*pane, session.check_memory());
            }
        }
    }
}
