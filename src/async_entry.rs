//! Asynchronous job entry types.
//!
//! This module defines the data structures and traits used to represent
//! scheduled async jobs in the asynchronous cron scheduler.

use std::{pin::Pin, sync::Arc};

use chrono::{DateTime, TimeZone};
use core::fmt;
use cron;
use futures::Future;

/// A type alias for boxed async task futures.
///
/// This represents the return type of async jobs that will be executed
/// by the async cron scheduler.
pub type TaskFuture = Box<dyn Future<Output = ()> + Send>;

/// Trait for types that can provide pinned futures for async execution.
///
/// This trait abstracts over the creation of pinned futures from
/// function objects, allowing the async scheduler to execute jobs
/// without knowing their specific implementation details.
pub trait TaskFuturePinned {
    /// Returns a pinned future that can be awaited.
    ///
    /// This method is called by the scheduler when it's time to execute
    /// the async job. The returned future will be spawned as a tokio task.
    fn get_pinned(&self) -> Pin<TaskFuture>;
}

/// A wrapper that converts async closures into executable tasks.
///
/// `TaskWrapper` takes a function that returns a Future and implements
/// the `TaskFuturePinned` trait, allowing it to be used by the async
/// cron scheduler.
///
/// # Type Parameters
///
/// * `F` - The function type that returns a Future
/// * `T` - The Future type returned by the function
///
/// # Note
/// 
/// This type is primarily used internally by the AsyncCron scheduler and is not
/// typically constructed directly by user code.
pub struct TaskWrapper<F, T>(F)
where
    F: Fn() -> T,
    T: Future;

impl<F, T> TaskWrapper<F, T>
where
    F: Fn() -> T,
    T: Future,
{
    /// Creates a new `TaskWrapper` from the given function.
    ///
    /// # Arguments
    ///
    /// * `f` - A function that returns a Future when called
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create an async cron scheduler to demonstrate task wrapping
    /// let mut cron = AsyncCron::new(Utc);
    /// 
    /// // Add an async job - TaskWrapper is used internally
    /// cron.add_fn("* * * * * * *", || async {
    ///     println!("Hello from async task!");
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(f: F) -> Self {
        TaskWrapper(f)
    }
}

impl<F, T> TaskFuturePinned for TaskWrapper<F, T>
where
    F: Fn() -> T,
    T: Future<Output = ()> + Send + 'static,
{
    /// Creates a pinned future by calling the wrapped function.
    ///
    /// This implementation calls the wrapped function to get a Future,
    /// then boxes and pins it for execution by the async runtime.
    fn get_pinned(&self) -> Pin<TaskFuture> {
        Box::pin(self.0())
    }
}

/// Represents a scheduled async job entry in the asynchronous cron scheduler.
///
/// An `AsyncEntry` contains all the information needed to execute an async job
/// according to its cron schedule, including the async task to execute, when
/// it should run next, and its scheduling pattern.
///
/// # Type Parameters
///
/// * `Z` - A timezone type that implements `TimeZone + Send + Sync + 'static`
///
/// # Note
/// 
/// This type is primarily used internally by the AsyncCron scheduler and is not
/// typically constructed directly by user code.
#[derive(Clone)]
pub struct AsyncEntry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    /// Unique identifier for this async job entry.
    ///
    /// This ID is used to remove or manage the job after it has been added
    /// to the async scheduler.
    pub id: usize,
    
    /// The cron schedule that determines when this async job should run.
    ///
    /// This uses the `cron::Schedule` type from the cron crate to parse
    /// and calculate execution times.
    pub schedule: cron::Schedule,
    
    /// The next scheduled execution time for this async job.
    ///
    /// This is calculated based on the cron schedule and current time.
    /// `None` indicates the job hasn't been scheduled yet.
    pub next: Option<DateTime<Z>>,
    
    /// The async task to execute when the job runs.
    ///
    /// This is an `Arc<dyn TaskFuturePinned>` to allow the task to be safely
    /// shared between async contexts and cloned when spawning execution tasks.
    pub run: Arc<dyn TaskFuturePinned + Send + Sync>,
}

impl<Z> fmt::Debug for AsyncEntry<Z>
where
    Z: TimeZone + Send + Sync + 'static,
    Z::Offset: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncEntry")
            .field("id", &self.id)
            .field("schedule", &self.schedule)
            .field("next", &self.next)
            .finish()
    }
}

impl<Z> AsyncEntry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    /// Calculates the next execution time for this async job based on its schedule.
    ///
    /// This method uses the cron schedule to determine when the async job should
    /// run next, taking into account the provided timezone.
    ///
    /// # Arguments
    ///
    /// * `tz` - The timezone to use for scheduling calculations
    ///
    /// # Returns
    ///
    /// Returns `Some(DateTime<Z>)` with the next execution time, or `None`
    /// if no future execution time can be determined.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use cron_tab::AsyncCron;
    /// use std::str::FromStr;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create an async cron scheduler to demonstrate scheduling
    /// let mut cron = AsyncCron::new(Utc);
    /// 
    /// // Add an async job that runs every hour
    /// let job_id = cron.add_fn("0 0 * * * * *", || async {
    ///     println!("Hourly async job executed!");
    /// }).await?;
    /// 
    /// // The scheduler internally calculates next execution times
    /// // This is typically handled automatically by the AsyncCron scheduler
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_next(&self, tz: Z) -> Option<DateTime<Z>> {
        self.schedule.upcoming(tz).next()
    }
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_async_entry_debug() {
        let entry: AsyncEntry<Utc> = AsyncEntry {
            id: 1,
            next: None,
            schedule: "* * * * * *".parse().unwrap(),
            run: Arc::new(TaskWrapper::new(|| async { })),
        };
        
        let debug_str = format!("{:?}", entry);
        assert!(debug_str.contains("AsyncEntry"));
        assert!(debug_str.contains("id: 1"));
    }

    #[tokio::test]
    async fn test_async_entry_get_next() {
        let entry: AsyncEntry<Utc> = AsyncEntry {
            id: 1,
            next: None,
            schedule: "* * * * * *".parse().unwrap(),
            run: Arc::new(TaskWrapper::new(|| async { })),
        };
        
        let now = Utc::now();
        let next = entry.get_next(Utc);
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[tokio::test]
    async fn test_task_wrapper() {
        let executed = Arc::new(Mutex::new(false));
        let executed_clone = Arc::clone(&executed);
        
        let wrapper = TaskWrapper::new(move || {
            let executed = executed_clone.clone();
            async move {
                *executed.lock().unwrap() = true;
            }
        });
        
        wrapper.get_pinned().await;
        assert!(*executed.lock().unwrap());
    }
}
