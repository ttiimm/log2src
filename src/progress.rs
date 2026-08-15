use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static GLOBAL_PROGRESS_TRACKER: OnceLock<Arc<ProgressTracker>> = OnceLock::new();

/// Sets the global progress tracker, replacing any previously registered one.
///
/// Returns the previous tracker if one was registered, or `None` if none was set.
///
/// The tracker is used by library operations (such as [`LogMatcher::discover_sources`]
/// and [`LogMatcher::extract_log_statements`]) to report progress. If no tracker is
/// registered, those operations run silently.
///
/// Call this once at application startup, before invoking any library operations.
/// The tracker remains active for the lifetime of the program unless replaced or
/// cleared. Replacing a tracker while an operation is in progress is safe — the
/// running operation captures the tracker at its start and will continue using
/// the previous one to completion.
///
/// # Example
/// ```no_run
/// use std::sync::Arc;
/// use log2src::{ProgressTracker, set_tracker_once};
///
/// let tracker = Arc::new(ProgressTracker::new());
/// set_tracker_once(Arc::clone(&tracker));
/// ```
pub fn set_tracker_once(tracker: Arc<ProgressTracker>) {
    let _ = GLOBAL_PROGRESS_TRACKER.set(tracker);
}

pub(crate) fn current_global_progress_tracker() -> Arc<ProgressTracker> {
    GLOBAL_PROGRESS_TRACKER
        .get()
        .cloned()
        .unwrap_or_default()
}

pub struct WorkInfo {
    pub completed: AtomicU64,
    pub total: u64,
    pub units: String,
}

impl WorkInfo {
    /// Check if the work is still in-progress.
    pub fn is_in_progress(&self) -> bool {
        self.completed.load(Ordering::Relaxed) < self.total
    }
}

/// A notification of progress for subscribers to a ProgressTracker
pub enum ProgressUpdate {
    /// A description of a large amount of work.
    Step(String),
    /// The start of a batch of work.
    BeginStep(String),
    /// The end of a batch of work.
    EndStep(String),
    /// A deterministic amount of work.
    Work(Arc<WorkInfo>),
}

pub struct ProgressListener {
    receiver: Receiver<ProgressUpdate>,
}

#[derive(Default, Debug)]
/// A mechanism for tracking progress.
pub struct ProgressTracker {
    subscribers: Vec<Sender<ProgressUpdate>>,
}

pub struct WorkGuard {
    info: Arc<WorkInfo>,
}

impl WorkGuard {
    /// Increase the amount of deterministic work that has been done.
    pub fn inc(&self, amount: u64) {
        self.info.completed.fetch_add(amount, Ordering::Relaxed);
    }
}

impl Drop for WorkGuard {
    fn drop(&mut self) {
        self.info
            .completed
            .store(self.info.total, Ordering::Relaxed);
    }
}

impl ProgressTracker {
    /// Create an empty tracker.
    pub fn new() -> ProgressTracker {
        ProgressTracker {
            subscribers: vec![],
        }
    }

    /// Notify subscribers of a step in a process.
    pub fn step(&self, message: String) {
        self.subscribers.iter().for_each(|sender| {
            let _ = sender.send(ProgressUpdate::Step(message.clone()));
        });
    }

    /// Notify subscribers of the beginning of a step in a process.
    pub fn begin_step(&self, message: String) {
        self.subscribers.iter().for_each(|sender| {
            let _ = sender.send(ProgressUpdate::BeginStep(message.clone()));
        });
    }

    pub fn end_step(&self, message: String) {
        self.subscribers.iter().for_each(|sender| {
            let _ = sender.send(ProgressUpdate::EndStep(message.clone()));
        });
    }

    /// Notify subscribers that some deterministic amount of work is about to be done.
    pub fn doing_work(&self, total: u64, units: String) -> WorkGuard {
        let info = Arc::new(WorkInfo {
            completed: AtomicU64::new(0),
            total,
            units,
        });

        self.subscribers.iter().for_each(|sender| {
            let _ = sender.send(ProgressUpdate::Work(Arc::clone(&info)));
        });

        WorkGuard {
            info: Arc::clone(&info),
        }
    }

    /// Subscribe to notifications of work for this tracker.
    pub fn subscribe(&mut self) -> ProgressListener {
        let (sender, receiver) = channel();

        self.subscribers.push(sender);
        ProgressListener { receiver }
    }
}

impl Iterator for ProgressListener {
    type Item = ProgressUpdate;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.iter().next()
    }
}

impl ProgressListener {
    pub fn try_next_for(&self, timeout: Duration) -> Option<ProgressUpdate> {
        self.receiver.recv_timeout(timeout).ok()
    }
}
