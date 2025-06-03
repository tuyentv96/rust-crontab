//! Asynchronous cron job scheduler implementation.
//!
//! This module provides a tokio-based async cron scheduler that executes jobs
//! as async tasks using the tokio runtime. Jobs run concurrently without blocking
//! the scheduler or each other.

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

/// An asynchronous cron job scheduler that manages and executes scheduled async jobs.
///
/// The `AsyncCron` struct provides an async-first approach to job scheduling using
/// tokio's runtime. Jobs are executed as async tasks, allowing for efficient
/// concurrent execution without blocking threads.
///
/// # Type Parameters
///
/// * `Z` - A timezone type that implements `TimeZone + Send + Sync + 'static`
///
/// # Async Runtime
///
/// This scheduler requires a tokio runtime to function. All methods are async
/// and jobs are executed as tokio tasks.
///
/// # Examples
///
/// ```rust
/// use chrono::Utc;
/// use cron_tab::AsyncCron;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut cron = AsyncCron::new(Utc);
/// 
/// let job_id = cron.add_fn("*/5 * * * * * *", || async {
///     println!("This async job runs every 5 seconds");
///     // Can perform async operations here
///     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
/// }).await?;
///
/// cron.start().await;
/// // Jobs will now execute according to their schedule
/// 
/// // Later, you can stop the scheduler
/// cron.stop().await;
/// # Ok(())
/// # }
/// ```
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

/// Implementation of the asynchronous cron scheduler.
impl<Z> AsyncCron<Z>
where
    Z: TimeZone + Send + Sync + 'static,
    Z::Offset: Send,
{
    /// Adds an async function to be executed according to the specified cron schedule.
    ///
    /// The function should return a Future that will be awaited when the job executes.
    /// This allows for true asynchronous job execution without blocking threads.
    ///
    /// # Arguments
    ///
    /// * `spec` - A cron expression string in the format "sec min hour day month weekday year"
    /// * `f` - A function that returns a Future implementing `Future<Output = ()> + Send + 'static`
    ///
    /// # Returns
    ///
    /// Returns a `Result<usize, CronError>` where the `usize` is a unique job ID
    /// that can be used with [`remove`](Self::remove) to cancel the job.
    ///
    /// # Errors
    ///
    /// Returns [`CronError::ParseError`](crate::CronError::ParseError) if the cron expression is invalid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut cron = AsyncCron::new(Utc);
    ///
    /// // Simple async job
    /// let job_id = cron.add_fn("*/10 * * * * * *", || async {
    ///     println!("Async job executed!");
    ///     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    /// }).await?;
    ///
    /// // Job with shared state
    /// let counter = Arc::new(Mutex::new(0));
    /// let counter_clone = counter.clone();
    /// cron.add_fn("* * * * * * *", move || {
    ///     let counter = counter_clone.clone();
    ///     async move {
    ///         let mut count = counter.lock().await;
    ///         *count += 1;
    ///         println!("Count: {}", *count);
    ///     }
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_fn<F, T>(&mut self, spec: &str, f: F) -> Result<usize>
    where
        F: 'static + Fn() -> T + Send + Sync,
        T: 'static + Future<Output = ()> + Send,
    {
        let schedule = cron::Schedule::from_str(spec)?;
        self.schedule(schedule, f).await
    }

    /// Returns a clone of the current timezone.
    fn get_timezone(&self) -> Z {
        self.tz.clone()
    }

    /// Creates a new async cron scheduler with the specified timezone.
    ///
    /// The scheduler is created in a stopped state. Call [`start`](Self::start) 
    /// to begin executing scheduled jobs.
    ///
    /// # Arguments
    ///
    /// * `tz` - The timezone to use for all scheduling calculations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::{Utc, FixedOffset};
    /// use cron_tab::AsyncCron;
    ///
    /// // UTC timezone
    /// let cron_utc = AsyncCron::new(Utc);
    ///
    /// // Fixed offset timezone (Tokyo: UTC+9)
    /// let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
    /// let cron_tokyo = AsyncCron::new(tokyo_tz);
    /// ```
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

    /// Returns the current time in the scheduler's timezone.
    fn now(&self) -> DateTime<Z> {
        self.get_timezone()
            .from_utc_datetime(&Utc::now().naive_utc())
    }

    /// Removes a job from the scheduler.
    ///
    /// Once removed, the job will no longer be executed. If the job ID doesn't exist,
    /// this method does nothing. If the scheduler is running, the removal is handled
    /// asynchronously via the scheduler's event loop.
    ///
    /// # Arguments
    ///
    /// * `id` - The job ID returned by [`add_fn`](Self::add_fn)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut cron = AsyncCron::new(Utc);
    /// let job_id = cron.add_fn("* * * * * * *", || async {
    ///     println!("This will be removed");
    /// }).await?;
    ///
    /// cron.start().await;
    /// 
    /// // Later, remove the job
    /// cron.remove(job_id).await;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Internal method to remove a job entry by ID.
    ///
    /// This method acquires a lock on the entries vector and removes the job
    /// with the matching ID if it exists.
    async fn remove_entry(&self, id: usize) {
        let mut entries = self.entries.lock().await;
        if let Some(index) = entries.iter().position(|e| e.id == id) {
            entries.remove(index);
        }
    }

    /// Runs the scheduler in the current task (blocking).
    ///
    /// This method runs the main scheduler loop in the current async context,
    /// blocking until [`stop`](Self::stop) is called. This is useful when you want
    /// to run the scheduler as the main task of your application.
    ///
    /// # Behavior
    ///
    /// The scheduler will:
    /// 1. Set up communication channels for job management
    /// 2. Calculate the next execution time for all jobs
    /// 3. Sleep until the next job is due
    /// 4. Execute all due jobs as async tasks
    /// 5. Handle job additions/removals during runtime
    /// 6. Repeat until stopped
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut cron = AsyncCron::new(Utc);
    ///     cron.add_fn("0 0 * * * * *", || async {
    ///         println!("Top of the hour!");
    ///     }).await?;
    ///
    ///     // This will block until stop() is called from another task
    ///     cron.start_blocking().await;
    ///     Ok(())
    /// }
    /// ```
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
                // Calculate wait time until the next job execution
                let wait_milis = (entry.next.as_ref().unwrap().timestamp_millis() as u64)
                    .saturating_sub(self.now().timestamp_millis() as u64);

                wait_duration = Duration::from_millis(wait_milis);
            }

            // Release the lock before waiting
            drop(entries);

            // Use `select!` to handle multiple asynchronous operations concurrently.
            select! {
                // Timer expired - check for jobs to execute
                _ = tokio_time::sleep(wait_duration) => {
                    let now = self.now();
                    for entry in self.entries.lock().await.iter_mut() {
                        // Stop when we reach jobs that aren't due yet
                        if entry.next.as_ref().unwrap().gt(&now) {
                            break;
                        }

                        // Spawn the job to run asynchronously as a tokio task
                        let run = entry.run.clone();
                        tokio::spawn(async move {
                            run.as_ref().get_pinned().await;
                        });

                        // Schedule the next run of the job.
                        entry.next = entry.get_next(self.get_timezone());
                    }
                },
                // New job added while running
                 new_entry = add_rx.recv() => {
                    let mut entry = new_entry.unwrap();
                    entry.next = entry.get_next(self.get_timezone());
                    self.entries.lock().await.push(entry);
                },
                // Job removal requested
                 id = remove_rx.recv() => {
                    self.remove_entry(id.unwrap()).await;
                },
                // Stop signal received
                _ = stop_rx.recv() => {
                    return;
                },
            }
        }
    }

    /// Internal method to schedule a job with a parsed cron schedule.
    ///
    /// This method generates a unique ID for the job and adds it to the scheduler.
    /// If the scheduler is running, the job is sent via the add channel, otherwise
    /// it's added directly to the entries list.
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

    /// Sets the timezone for the scheduler.
    ///
    /// This affects how cron expressions are interpreted for all future job executions.
    /// Existing jobs will use the new timezone for their next scheduled execution.
    ///
    /// # Arguments
    ///
    /// * `tz` - The new timezone to use
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::{Utc, FixedOffset};
    /// use cron_tab::AsyncCron;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create async cron with UTC timezone
    /// let mut cron_utc = AsyncCron::new(Utc);
    ///
    /// // Create a separate async cron with Tokyo timezone  
    /// let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
    /// let mut cron_tokyo = AsyncCron::new(tokyo_tz);
    ///
    /// // Each scheduler uses its own timezone for job scheduling
    /// cron_utc.add_fn("0 0 12 * * * *", || async {
    ///     println!("Noon UTC");
    /// }).await?;
    ///
    /// cron_tokyo.add_fn("0 0 12 * * * *", || async {
    ///     println!("Noon Tokyo time");
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_timezone(&mut self, tz: Z) {
        self.tz = tz;
    }

    /// Starts the cron scheduler in a background task.
    ///
    /// This method spawns a new tokio task that will continuously monitor for jobs
    /// that need to be executed and spawn additional tasks to run them.
    /// The method returns immediately, allowing your program to continue.
    ///
    /// # Async Runtime
    ///
    /// The scheduler runs as a tokio task and spawns additional tasks for
    /// each job execution. This ensures that long-running async jobs don't block
    /// the scheduler or other jobs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut cron = AsyncCron::new(Utc);
    /// cron.add_fn("*/2 * * * * * *", || async {
    ///     println!("Job executed every 2 seconds");
    ///     // Can perform async operations here
    ///     tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    /// }).await?;
    ///
    /// // Start the scheduler
    /// cron.start().await;
    ///
    /// // The current task can continue with other work
    /// tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    ///
    /// // Stop the scheduler
    /// cron.stop().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&mut self) {
        let mut cloned = self.clone();
        self.running.store(true, Ordering::SeqCst);
        tokio::spawn(async move {
            cloned.start_blocking().await;
        });
    }

    /// Stops the cron scheduler.
    ///
    /// This sends a stop signal to the scheduler task, causing it to exit gracefully.
    /// Any currently executing async jobs will continue to completion, but no new jobs
    /// will be started.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut cron = AsyncCron::new(Utc);
    /// cron.add_fn("* * * * * * *", || async {
    ///     println!("Hello async world!");
    /// }).await?;
    /// cron.start().await;
    ///
    /// // Later, stop the scheduler
    /// cron.stop().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(tx) = self.stop_tx.lock().await.as_ref() {
            let _ = tx.send(true);
        }
    }
}
