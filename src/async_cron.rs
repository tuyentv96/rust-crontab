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

#[derive(Clone)]
pub struct AsyncCron<Z>
where
    Z: TimeZone + Send + Sync + 'static,
    Z::Offset: Send,
{
    entries: Arc<Mutex<Vec<AsyncEntry<Z>>>>,
    next_id: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    tz: Z,
    add_tx: Arc<Mutex<Option<mpsc::UnboundedSender<AsyncEntry<Z>>>>>,
    remove_tx: Arc<Mutex<Option<mpsc::UnboundedSender<usize>>>>,
    stop_tx: Arc<Mutex<Option<mpsc::UnboundedSender<bool>>>>,
}

/// Cron contains and executes the scheduled jobs.
impl<Z> AsyncCron<Z>
where
    Z: TimeZone + Send + Sync + 'static,
    Z::Offset: Send,
{
    /// Add a function to Cron.
    pub async fn add_fn<F, T>(&mut self, spec: &str, f: F) -> Result<usize>
    where
        F: 'static + Fn() -> T + Send + Sync,
        T: 'static + Future<Output = ()> + Send + Sync,
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

    /// Run a blocking loop for schedule jobs
    pub async fn start_blocking(&mut self) {
        let (add_tx, mut add_rx) = mpsc::unbounded_channel();
        let (remove_tx, mut remove_rx) = mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = mpsc::unbounded_channel();

        {
            *self.add_tx.lock().await = Some(add_tx);
            *self.remove_tx.lock().await = Some(remove_tx);
            *self.stop_tx.lock().await = Some(stop_tx);
        }

        self.running.store(true, Ordering::SeqCst);

        for entry in self.entries.lock().await.iter_mut() {
            entry.next = entry.get_next(self.get_timezone());
        }

        // default long timer duration
        let mut wait_duration = Duration::from_secs(MAX_WAIT_SECONDS);

        loop {
            let mut entries = self.entries.lock().await;
            entries.sort_by(|b, a| b.next.cmp(&a.next));

            if let Some(entry) = entries.first() {
                // get first entry from sorted entries for timer duration
                let wait_milis = (entry.next.as_ref().unwrap().timestamp_millis() as u64)
                    .saturating_sub(self.now().timestamp_millis() as u64);

                wait_duration = Duration::from_millis(wait_milis);
            }

            // release lock
            drop(entries);

            select! {
                // sleep and wait until next scheduled time
                _ = tokio_time::sleep(wait_duration) => {
                    let now = self.now();
                    for entry in self.entries.lock().await.iter_mut() {
                        if entry.next.as_ref().unwrap().gt(&now) {
                            break;
                        }

                        let run = entry.run.clone();
                        tokio::spawn(async move {
                            run.as_ref().get_pinned().await;
                        });

                        entry.next = entry.get_next(self.get_timezone());
                    }
                },
                // wait new entry added signal
                 new_entry = add_rx.recv() => {
                    let mut entry = new_entry.unwrap();
                    entry.next = entry.get_next(self.get_timezone());
                    self.entries.lock().await.push(entry);
                },
                // wait entry removed signal
                 id = remove_rx.recv() => {
                    self.remove_entry(id.unwrap()).await;
                },
                // wait cron stopped signal
                _ = stop_rx.recv() => {
                    return;
                },
            }
        }
    }

    async fn schedule<F, T>(&mut self, schedule: cron::Schedule, f: F) -> Result<usize>
    where
        F: 'static + Fn() -> T + Send + Sync,
        T: 'static + Future<Output = ()> + Send + Sync,
    {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let mut entry = AsyncEntry {
            id: next_id,
            schedule,
            next: None,
            run: Arc::new(TaskWrapper::new(f)),
        };

        entry.next = entry.get_next(self.get_timezone());

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

    /// Start cron in background.
    /// A toki stask will be spawn for schedule jobs
    pub async fn start(&mut self) {
        let mut cloned = self.clone();
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
