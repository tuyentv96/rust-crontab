//! Synchronous cron job scheduler implementation.
//!
//! This module provides a thread-based cron scheduler that executes jobs
//! in separate threads as they become due according to their cron schedule.

use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};

use crate::entry::Entry;
use crate::Result;
use crate::MAX_WAIT_SECONDS;

/// A synchronous cron job scheduler that manages and executes scheduled jobs.
///
/// The `Cron` struct provides a thread-safe way to schedule jobs using cron expressions.
/// Jobs are executed in separate threads as they become due, allowing for concurrent
/// execution without blocking the scheduler.
///
/// # Type Parameters
///
/// * `Z` - A timezone type that implements `TimeZone + Sync + Send + 'static`
///
/// # Thread Safety
///
/// This struct is thread-safe and can be safely shared between threads. All jobs
/// are executed in separate threads, and the scheduler itself runs in a background thread.
///
/// # Examples
///
/// ```rust
/// use chrono::Utc;
/// use cron_tab::Cron;
///
/// let mut cron = Cron::new(Utc);
/// let job_id = cron.add_fn("*/5 * * * * * *", || {
///     println!("This job runs every 5 seconds");
/// }).unwrap();
///
/// cron.start();
/// // Jobs will now execute according to their schedule
/// 
/// // Later, you can stop the scheduler
/// cron.stop();
/// ```
#[derive(Clone, Debug)]
pub struct Cron<Z>
where
    Z: TimeZone + Sync + Send + 'static,
    Z::Offset: Send,
{
    /// Thread-safe storage for all scheduled job entries
    entries: Arc<Mutex<Vec<Entry<Z>>>>,
    /// Atomic counter for generating unique job IDs
    next_id: Arc<AtomicUsize>,
    /// The timezone used for scheduling calculations
    tz: Z,
    /// Channel for adding new jobs to the running scheduler
    add_channel: (
        crossbeam_channel::Sender<Entry<Z>>,
        crossbeam_channel::Receiver<Entry<Z>>,
    ),
    /// Channel for stopping the scheduler
    stop_channel: (
        crossbeam_channel::Sender<bool>,
        crossbeam_channel::Receiver<bool>,
    ),
}

/// Implementation of the synchronous cron scheduler.
impl<Z> Cron<Z>
where
    Z: TimeZone + Sync + Send + 'static,
    Z::Offset: Send,
{
    /// Creates a new cron scheduler with the specified timezone.
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
    /// use cron_tab::Cron;
    ///
    /// // UTC timezone
    /// let cron_utc = Cron::new(Utc);
    ///
    /// // Fixed offset timezone (Tokyo: UTC+9)
    /// let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
    /// let cron_tokyo = Cron::new(tokyo_tz);
    /// ```
    pub fn new(tz: Z) -> Cron<Z> {
        Cron {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicUsize::new(0)),
            tz,
            add_channel: crossbeam_channel::unbounded(),
            stop_channel: crossbeam_channel::unbounded(),
        }
    }

    /// Adds a function to be executed according to the specified cron schedule.
    ///
    /// The function will be called without arguments whenever the cron expression
    /// matches the current time in the scheduler's timezone.
    ///
    /// # Arguments
    ///
    /// * `spec` - A cron expression string in the format "sec min hour day month weekday year"
    /// * `f` - A function that implements `Fn() + Send + Sync + 'static`
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
    /// use cron_tab::Cron;
    ///
    /// let mut cron = Cron::new(Utc);
    ///
    /// // Run every second
    /// let job_id = cron.add_fn("* * * * * * *", || {
    ///     println!("Every second!");
    /// }).unwrap();
    ///
    /// // Run every day at 9:00 AM
    /// let morning_job = cron.add_fn("0 0 9 * * * *", || {
    ///     println!("Good morning!");
    /// }).unwrap();
    ///
    /// // Run every Monday at 10:30 AM
    /// let monday_job = cron.add_fn("0 30 10 * * MON *", || {
    ///     println!("Monday meeting reminder!");
    /// }).unwrap();
    /// ```
    pub fn add_fn<T>(&mut self, spec: &str, f: T) -> Result<usize>
    where
        T: 'static,
        T: Fn() + Send + Sync,
    {
        let schedule = cron::Schedule::from_str(spec)?;
        self.schedule(schedule, f)
    }

    /// Adds a function to be executed once at a specific datetime.
    ///
    /// The function will be called exactly once when the specified time is reached.
    /// After execution, the job is automatically removed from the scheduler.
    ///
    /// # Arguments
    ///
    /// * `datetime` - The specific time when the job should execute
    /// * `f` - A function that implements `Fn() + Send + Sync + 'static`
    ///
    /// # Returns
    ///
    /// Returns a `Result<usize, CronError>` where the `usize` is a unique job ID
    /// that can be used with [`remove`](Self::remove) to cancel the job.
    ///
    /// # Behavior with Past Times
    ///
    /// If the specified datetime is in the past, the job will execute immediately
    /// on the next scheduler iteration. This allows for jobs that may have been
    /// scheduled while the system was offline or during startup.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::{Utc, Duration};
    /// use cron_tab::Cron;
    ///
    /// let mut cron = Cron::new(Utc);
    ///
    /// // Execute once at a specific time
    /// let target_time = Utc::now() + Duration::seconds(10);
    /// let job_id = cron.add_fn_once(target_time, || {
    ///     println!("This runs once at the specified time!");
    /// }).unwrap();
    /// ```
    pub fn add_fn_once<T>(&mut self, datetime: DateTime<Z>, f: T) -> Result<usize>
    where
        T: 'static,
        T: Fn() + Send + Sync,
    {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let entry = Entry {
            id: next_id,
            next: Some(datetime),
            run: Arc::new(f),
            schedule: None,
        };

        self.add_channel.0.send(entry).unwrap();
        Ok(next_id)
    }

    /// Adds a function to be executed once after a specified delay.
    ///
    /// The function will be called exactly once after the specified duration has passed.
    /// After execution, the job is automatically removed from the scheduler.
    ///
    /// # Arguments
    ///
    /// * `delay` - The duration to wait before executing the job
    /// * `f` - A function that implements `Fn() + Send + Sync + 'static`
    ///
    /// # Returns
    ///
    /// Returns a `Result<usize, CronError>` where the `usize` is a unique job ID
    /// that can be used with [`remove`](Self::remove) to cancel the job.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use chrono::Utc;
    /// use cron_tab::Cron;
    ///
    /// let mut cron = Cron::new(Utc);
    ///
    /// // Execute once after 30 seconds
    /// let job_id = cron.add_fn_after(Duration::from_secs(30), || {
    ///     println!("This runs once after 30 seconds!");
    /// }).unwrap();
    ///
    /// // Execute after 5 minutes
    /// let delayed_job = cron.add_fn_after(Duration::from_secs(300), || {
    ///     println!("This runs after 5 minutes!");
    /// }).unwrap();
    /// ```
    pub fn add_fn_after<T>(&mut self, delay: Duration, f: T) -> Result<usize>
    where
        T: 'static,
        T: Fn() + Send + Sync,
    {
        let chrono_delay = chrono::Duration::from_std(delay)
            .map_err(|_| crate::CronError::DurationOutOfRange)?;
        let execute_at = self.now() + chrono_delay;
        self.add_fn_once(execute_at, f)
    }

    /// Internal method to schedule a job with a parsed cron schedule.
    ///
    /// This method generates a unique ID for the job and adds it to the scheduler.
    /// If the scheduler is already running, the job is sent via the add channel.
    fn schedule<T>(&mut self, schedule: cron::Schedule, f: T) -> Result<usize>
    where
        T: Send + Sync + 'static,
        T: Fn(),
    {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let mut entry = Entry {
            id: next_id,
            next: None,
            run: Arc::new(f),
            schedule: Some(schedule),
        };

        entry.next = entry.schedule_next(self.get_timezone());
        self.add_channel.0.send(entry).unwrap();

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
    /// use cron_tab::Cron;
    ///
    /// // Create cron with UTC timezone
    /// let mut cron_utc = Cron::new(Utc);
    ///
    /// // Create a separate cron with Tokyo timezone  
    /// let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
    /// let mut cron_tokyo = Cron::new(tokyo_tz);
    ///
    /// // Each scheduler uses its own timezone for job scheduling
    /// cron_utc.add_fn("0 0 12 * * * *", || {
    ///     println!("Noon UTC");
    /// }).unwrap();
    ///
    /// cron_tokyo.add_fn("0 0 12 * * * *", || {
    ///     println!("Noon Tokyo time");
    /// }).unwrap();
    /// ```
    pub fn set_timezone(&mut self, tz: Z) {
        self.tz = tz;
    }

    /// Returns a clone of the current timezone.
    fn get_timezone(&self) -> Z {
        self.tz.clone()
    }

    /// Returns the current time in the scheduler's timezone.
    fn now(&self) -> DateTime<Z> {
        self.get_timezone()
            .from_utc_datetime(&Utc::now().naive_utc())
    }

    /// Internal method to remove a job entry by ID.
    ///
    /// This method acquires a lock on the entries vector and removes the job
    /// with the matching ID if it exists.
    fn remove_entry(&self, id: usize) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(index) = entries.iter().position(|e| e.id == id) {
            entries.remove(index);
        }
    }

    /// Removes a job from the scheduler.
    ///
    /// Once removed, the job will no longer be executed. If the job ID doesn't exist,
    /// this method does nothing.
    ///
    /// # Arguments
    ///
    /// * `id` - The job ID returned by [`add_fn`](Self::add_fn)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::Cron;
    ///
    /// let mut cron = Cron::new(Utc);
    /// let job_id = cron.add_fn("* * * * * * *", || {
    ///     println!("This will be removed");
    /// }).unwrap();
    ///
    /// cron.start();
    /// 
    /// // Later, remove the job
    /// cron.remove(job_id);
    /// ```
    pub fn remove(&self, id: usize) {
        self.remove_entry(id)
    }

    /// Stops the cron scheduler.
    ///
    /// This sends a stop signal to the scheduler thread, causing it to exit gracefully.
    /// Any currently executing jobs will continue to completion, but no new jobs
    /// will be started.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::Cron;
    ///
    /// let mut cron = Cron::new(Utc);
    /// cron.add_fn("* * * * * * *", || println!("Hello")).unwrap();
    /// cron.start();
    ///
    /// // Later, stop the scheduler
    /// cron.stop();
    /// ```
    pub fn stop(&self) {
        self.stop_channel.0.send(true).unwrap()
    }

    /// Starts the cron scheduler in a background thread.
    ///
    /// This method spawns a new thread that will continuously monitor for jobs
    /// that need to be executed and spawn additional threads to run them.
    /// The method returns immediately, allowing your program to continue.
    ///
    /// # Thread Safety
    ///
    /// The scheduler runs in its own thread and spawns additional threads for
    /// each job execution. This ensures that long-running jobs don't block
    /// the scheduler or other jobs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::Cron;
    /// use std::time::Duration;
    ///
    /// let mut cron = Cron::new(Utc);
    /// cron.add_fn("*/2 * * * * * *", || {
    ///     println!("Job executed every 2 seconds");
    /// }).unwrap();
    ///
    /// // Start the scheduler
    /// cron.start();
    ///
    /// // The main thread can continue with other work
    /// std::thread::sleep(Duration::from_secs(10));
    ///
    /// // Stop the scheduler
    /// cron.stop();
    /// ```
    pub fn start(&mut self) {
        let mut cron = self.clone();

        thread::spawn(move || {
            cron.start_blocking();
        });
    }

    /// Runs the scheduler in the current thread (blocking).
    ///
    /// This method runs the main scheduler loop in the current thread, making it
    /// block until [`stop`](Self::stop) is called. This is useful when you want
    /// to run the scheduler as the main thread of your application.
    ///
    /// # Behavior
    ///
    /// The scheduler will:
    /// 1. Calculate the next execution time for all jobs
    /// 2. Sleep until the next job is due
    /// 3. Execute all due jobs in separate threads
    /// 4. Repeat until stopped
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chrono::Utc;
    /// use cron_tab::Cron;
    ///
    /// let mut cron = Cron::new(Utc);
    /// cron.add_fn("0 0 * * * * *", || {
    ///     println!("Top of the hour!");
    /// }).unwrap();
    ///
    /// // This will block until stop() is called from another thread
    /// cron.start_blocking();
    /// ```
    pub fn start_blocking(&mut self) {
        // Initialize next execution times for entries that don't have one yet
        for entry in self.entries.lock().unwrap().iter_mut() {
            if entry.next.is_none() {
                entry.next = entry.schedule_next(self.get_timezone());
            }
        }

        // Default long timer duration for when no jobs are scheduled
        let mut wait_duration = Duration::from_secs(MAX_WAIT_SECONDS);

        loop {
            let mut entries = self.entries.lock().unwrap();
            entries.sort_by(|b, a| b.next.cmp(&a.next));

            if let Some(entry) = entries.first() {
                // Calculate wait time until the next job execution
                let wait_milis = (entry.next.as_ref().unwrap().timestamp_millis() as u64)
                    .saturating_sub(self.now().timestamp_millis() as u64);

                wait_duration = Duration::from_millis(wait_milis);
            }

            // Release the lock before waiting
            drop(entries);

            // Use crossbeam's select! to handle multiple channels concurrently
            crossbeam_channel::select! {
                // Timer expired - check for jobs to execute
                recv(crossbeam_channel::after(wait_duration)) -> _ => {
                    let now = self.now();
                    let mut entries = self.entries.lock().unwrap();
                    let mut jobs_to_remove = Vec::new();

                    for entry in entries.iter_mut() {
                        // Stop when we reach jobs that aren't due yet
                        if entry.next.as_ref().unwrap().gt(&now) {
                            break;
                        }

                        // Execute the job in a separate thread
                        let run = entry.run.clone();
                        thread::spawn(move || {
                            run();
                        });

                        // Mark one-time jobs for removal
                        if entry.is_once() {
                            jobs_to_remove.push(entry.id);
                        } else {
                            // Schedule the next execution for recurring jobs
                            entry.next = entry.schedule_next(self.get_timezone());
                        }
                    }

                    // Remove one-time jobs that have been executed
                    entries.retain(|e| !jobs_to_remove.contains(&e.id));
                },
                // New job added while running
                recv(self.add_channel.1) -> new_entry => {
                    let mut entry = new_entry.unwrap();
                    if entry.next.is_none() {
                        entry.next = entry.schedule_next(self.get_timezone());
                    }
                    self.entries.lock().unwrap().push(entry);
                },
                // Stop signal received
                recv(self.stop_channel.1) -> _ => {
                    return;
                },
            }
        }
    }
}
