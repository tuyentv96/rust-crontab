use std::future::Future;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use tokio::select;
use tokio::sync::{mpsc, Mutex};
use tokio::time as tokio_time;

use crate::async_entry::{AsyncEntry, TaskWrapper};
use crate::{Result, MAX_WAIT_SECONDS};

/// The `AsyncCron` struct manages scheduled jobs that run asynchronously.
/// It holds the job entries, keeps track of job IDs, and manages the state
/// of the running cron.
#[derive(Clone, Debug)]
pub struct AsyncCron<Z>
where
    Z: TimeZone + Send + Sync + 'static,
    Z::Offset: Send,
{
    /// A thread-safe, asynchronous list of job entries (schedules and tasks).
    entries: Arc<Mutex<Vec<AsyncEntry<Z>>>>,

    /// A counter for assigning unique IDs to job entries.
    next_id: Arc<AtomicUsize>,

    /// Indicates whether the cron is currently running.
    running: Arc<AtomicBool>,

    /// The timezone used for scheduling tasks.
    tz: Z,

    /// A channel sender for adding new entries to the cron scheduler.
    add_tx: Arc<Mutex<Option<mpsc::UnboundedSender<AsyncEntry<Z>>>>>,

    /// A channel sender for removing entries from the cron scheduler.
    remove_tx: Arc<Mutex<Option<mpsc::UnboundedSender<usize>>>>,

    /// A channel sender for stopping the cron scheduler.
    stop_tx: Arc<Mutex<Option<mpsc::UnboundedSender<bool>>>>,
}

/// Implementation of the `AsyncCron` struct, which provides methods for managing scheduled tasks.
impl<Z> AsyncCron<Z>
where
    Z: TimeZone + Send + Sync + 'static,
    Z::Offset: Send,
{
    /// Add a function to Cron.
    pub async fn add_fn<F, T>(&mut self, spec: &str, f: F) -> Result<usize>
    where
        F: 'static + Fn() -> T + Send + Sync,
        T: 'static + Future<Output = ()> + Send,
    {
        let schedule = cron::Schedule::from_str(spec)?;
        self.schedule(schedule, f).await
    }

    fn get_timezone(&self) -> Z {
        self.tz.clone()
    }

    /// Create a new cron.
    pub fn new(tz: Z) -> AsyncCron<Z> {
        AsyncCron {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicUsize::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            tz,
            add_tx: Default::default(),
            remove_tx: Default::default(),
            stop_tx: Default::default(),
        }
    }

    fn now(&self) -> DateTime<Z> {
        self.get_timezone()
            .from_utc_datetime(&Utc::now().naive_utc())
    }

    /// Remove a job from Cron.
    pub async fn remove(&self, id: usize) {
        if self.running.load(Ordering::SeqCst) {
            let guard = self.remove_tx.lock().await;
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(id);
            }
            return;
        }

        self.remove_entry(id).await;
    }

    async fn remove_entry(&self, id: usize) {
        let mut entries = self.entries.lock().await;
        if let Some(index) = entries.iter().position(|e| e.id == id) {
            entries.remove(index);
        }
    }

    /// Starts the cron scheduler in a blocking loop, processing scheduled jobs.
    /// The loop will sleep until the next scheduled job is ready to run, and it
    /// will handle adding, removing, and stopping jobs.
    pub async fn start_blocking(&mut self) {
        // Channels for communicating with the cron loop (adding/removing/stopping jobs).
        let (add_tx, mut add_rx) = mpsc::unbounded_channel();
        let (remove_tx, mut remove_rx) = mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = mpsc::unbounded_channel();

        {
            *self.add_tx.lock().await = Some(add_tx);
            *self.remove_tx.lock().await = Some(remove_tx);
            *self.stop_tx.lock().await = Some(stop_tx);
        }

        // Initialize the next scheduled time for all entries.
        for entry in self.entries.lock().await.iter_mut() {
            entry.next = entry.get_next(self.get_timezone());
        }

        // Set a default long wait duration for sleeping.
        let mut wait_duration = Duration::from_secs(MAX_WAIT_SECONDS);

        loop {
            // Lock and sort entries to prioritize the closest scheduled job.
            let mut entries = self.entries.lock().await;
            entries.sort_by(|b, a| b.next.cmp(&a.next));

            // Determine the wait duration based on the next scheduled job.
            if let Some(entry) = entries.first() {
                // get first entry from sorted entries for timer duration
                let wait_milis = (entry.next.as_ref().unwrap().timestamp_millis() as u64)
                    .saturating_sub(self.now().timestamp_millis() as u64);

                wait_duration = Duration::from_millis(wait_milis);
            }

            // release lock
            drop(entries);

            // Use `select!` to handle multiple asynchronous tasks concurrently.
            select! {
                // Sleep until the next scheduled job is ready to run.
                _ = tokio_time::sleep(wait_duration) => {
                    let now = self.now();
                    for entry in self.entries.lock().await.iter_mut() {
                        if entry.next.as_ref().unwrap().gt(&now) {
                            break;
                        }

                        // Spawn the job to run asynchronously.
                        let run = entry.run.clone();
                        tokio::spawn(async move {
                            run.as_ref().get_pinned().await;
                        });

                        // Schedule the next run of the job.
                        entry.next = entry.get_next(self.get_timezone());
                    }
                },
                // Add a new entry to the scheduler.
                 new_entry = add_rx.recv() => {
                    let mut entry = new_entry.unwrap();
                    entry.next = entry.get_next(self.get_timezone());
                    self.entries.lock().await.push(entry);
                },
                // Remove an entry from the scheduler by ID.
                 id = remove_rx.recv() => {
                    self.remove_entry(id.unwrap()).await;
                },
                // Stop the cron scheduler.
                _ = stop_rx.recv() => {
                    return;
                },
            }
        }
    }

    /// Schedules a new job by creating an `AsyncEntry` and adding it to the scheduler.
    /// If the scheduler is running, the job is added via the channel; otherwise, it's added directly.
    async fn schedule<F, T>(&mut self, schedule: cron::Schedule, f: F) -> Result<usize>
    where
        F: 'static + Fn() -> T + Send + Sync,
        T: 'static + Future<Output = ()> + Send,
    {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let mut entry = AsyncEntry {
            id: next_id,
            schedule,
            next: None,
            run: Arc::new(TaskWrapper::new(f)),
        };

        // Determine the next scheduled time for the job.
        entry.next = entry.get_next(self.get_timezone());

        // If the cron is running, send the entry via the channel; otherwise, add it directly.
        match self.add_tx.lock().await.as_ref() {
            Some(tx) if self.running.load(Ordering::SeqCst) => tx.send(entry).unwrap(),
            _ => self.entries.lock().await.push(entry),
        }

        Ok(next_id)
    }

    /// Set timezone offset.
    pub fn set_timezone(&mut self, tz: Z) {
        self.tz = tz;
    }

    /// Starts the cron scheduler in the background.
    /// A separate task is spawned to handle scheduled jobs asynchronously.
    pub async fn start(&mut self) {
        let mut cloned = self.clone();
        self.running.store(true, Ordering::SeqCst);
        tokio::spawn(async move {
            cloned.start_blocking().await;
        });
    }

    /// Stop Cron.
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(tx) = self.stop_tx.lock().await.as_ref() {
            let _ = tx.send(true);
        }
    }
}
