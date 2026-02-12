//! Synchronous job entry types.
//!
//! This module defines the data structures used to represent scheduled jobs
//! in the synchronous cron scheduler.

use std::{fmt, sync::Arc};

use chrono::{DateTime, TimeZone};

/// Represents a scheduled job entry in the synchronous cron scheduler.
///
/// An `Entry` contains all the information needed to execute a job according
/// to its cron schedule, including the function to execute, when it should
/// run next, and its scheduling pattern.
///
/// # Type Parameters
///
/// * `Z` - A timezone type that implements `TimeZone + Sync + Send + 'static`
///
/// # Note
///
/// This type is primarily used internally by the Cron scheduler and is not
/// typically constructed directly by user code.
#[derive(Clone)]
pub struct Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
{
    /// Unique identifier for this job entry.
    ///
    /// This ID is used to remove or manage the job after it has been added
    /// to the scheduler.
    pub id: usize,

    /// The next scheduled execution time for this job.
    ///
    /// This is calculated based on the cron schedule and current time.
    /// `None` indicates the job hasn't been scheduled yet.
    pub next: Option<DateTime<Z>>,

    /// The function to execute when the job runs.
    ///
    /// This is an `Arc<dyn Fn()>` to allow the function to be safely shared
    /// between threads and cloned when spawning execution threads.
    pub run: Arc<dyn Fn() + Send + Sync + 'static>,

    /// The cron schedule that determines when this job should run.
    ///
    /// This uses the `cron::Schedule` type from the cron crate to parse
    /// and calculate execution times.
    /// `None` for one-time jobs that execute once and are automatically removed.
    pub schedule: Option<cron::Schedule>,
}

impl<Z> fmt::Debug for Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
    Z::Offset: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("next", &self.next)
            .field("schedule", &self.schedule)
            .finish()
    }
}

impl<Z> Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
{
    /// Calculates the next execution time for this job based on its schedule.
    ///
    /// This method uses the cron schedule to determine when the job should
    /// run next, taking into account the provided timezone.
    ///
    /// # Arguments
    ///
    /// * `tz` - The timezone to use for scheduling calculations
    ///
    /// # Returns
    ///
    /// Returns `Some(DateTime<Z>)` with the next execution time, or `None`
    /// if no future execution time can be determined (e.g. one-time jobs with no schedule).
    pub fn schedule_next(&self, tz: Z) -> Option<DateTime<Z>> {
        self.schedule.as_ref().and_then(|s| s.upcoming(tz).next())
    }

    /// Returns `true` if this is a one-time job (no cron schedule).
    pub fn is_once(&self) -> bool {
        self.schedule.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::Arc;

    #[test]
    fn test_entry_debug() {
        let entry: Entry<Utc> = Entry {
            id: 1,
            next: None,
            run: Arc::new(|| {}),
            schedule: Some("* * * * * *".parse().unwrap()),
        };

        let debug_str = format!("{:?}", entry);
        assert!(debug_str.contains("Entry"));
        assert!(debug_str.contains("id: 1"));
    }

    #[test]
    fn test_entry_schedule_next() {
        let entry: Entry<Utc> = Entry {
            id: 1,
            next: None,
            run: Arc::new(|| {}),
            schedule: Some("* * * * * *".parse().unwrap()),
        };

        let now = Utc::now();
        let next = entry.schedule_next(Utc);
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[test]
    fn test_entry_clone() {
        let entry: Entry<Utc> = Entry {
            id: 1,
            next: None,
            run: Arc::new(|| {}),
            schedule: Some("* * * * * *".parse().unwrap()),
        };

        let cloned = entry.clone();
        assert_eq!(cloned.id, entry.id);
    }

    #[test]
    fn test_entry_once() {
        let entry: Entry<Utc> = Entry {
            id: 1,
            next: Some(Utc::now()),
            run: Arc::new(|| {}),
            schedule: None,
        };

        // One-time jobs should not reschedule
        let next = entry.schedule_next(Utc);
        assert!(next.is_none());
    }
}
