//! # CronTab - A Modern Rust Cron Job Scheduler
//!
//! CronTab is a feature-rich cron job scheduling library for Rust applications that provides
//! both synchronous and asynchronous job execution with timezone support and high performance.
//!
//! ## Features
//!
//! - **Dual Execution Modes**: Support for both synchronous and asynchronous job execution
//! - **Timezone Support**: Full timezone handling with chrono integration
//! - **High Performance**: Efficient job scheduling with minimal overhead
//! - **Thread Safety**: Built with Rust's safety guarantees in mind
//! - **Flexible Scheduling**: Standard cron expressions with second precision
//! - **Runtime Control**: Add, remove, start, and stop jobs at runtime
//!
//! ## Installation
//!
//! Add CronTab to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! # For both sync and async support
//! cron_tab = { version = "0.2", features = ["sync", "async"] }
//!
//! # For sync only
//! cron_tab = { version = "0.2", features = ["sync"] }
//!
//! # For async only
//! cron_tab = { version = "0.2", features = ["async"] }
//! ```
//!
//! ## Cron Expression Format
//!
//! CronTab supports 7-field cron expressions with second precision:
//!
//! ```text
//! ┌───────────── second (0 - 59)
//! │ ┌─────────── minute (0 - 59)
//! │ │ ┌───────── hour (0 - 23)
//! │ │ │ ┌─────── day of month (1 - 31)
//! │ │ │ │ ┌───── month (1 - 12)
//! │ │ │ │ │ ┌─── day of week (0 - 6) (Sunday to Saturday)
//! │ │ │ │ │ │ ┌─ year (1970 - 3000)
//! │ │ │ │ │ │ │
//! * * * * * * *
//! ```
//!
//! ## Synchronous Usage Example
//!
//! ```rust
//! use chrono::{FixedOffset, Local, TimeZone, Utc};
//! use cron_tab::Cron;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new cron scheduler with UTC timezone
//! let mut cron = Cron::new(Utc);
//!
//! // Add a job that runs every 10 seconds
//! let job_id = cron.add_fn("*/10 * * * * * *", || {
//!     println!("Job executed at: {}", Local::now());
//! })?;
//!
//! // Start the scheduler in background
//! cron.start();
//!
//! // Add another job that runs every minute
//! cron.add_fn("0 * * * * * *", || {
//!     println!("Every minute job executed!");
//! })?;
//!
//! // Remove the first job after some time
//! std::thread::sleep(std::time::Duration::from_secs(1));
//! cron.remove(job_id);
//!
//! // Stop the scheduler
//! cron.stop();
//! # Ok(())
//! # }
//! ```
//!
//! ## Asynchronous Usage Example
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! # {
//! use std::sync::Arc;
//! use chrono::{FixedOffset, Local, TimeZone, Utc};
//! use cron_tab::AsyncCron;
//! use tokio::sync::Mutex;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new async cron scheduler
//! let mut cron = AsyncCron::new(Utc);
//!
//! // Shared state between jobs
//! let counter = Arc::new(Mutex::new(0));
//!
//! // Add an async job that increments counter
//! let counter_clone = counter.clone();
//! cron.add_fn("* * * * * * *", move || {
//!     let counter = counter_clone.clone();
//!     async move {
//!         let mut count = counter.lock().await;
//!         *count += 1;
//!         println!("Counter: {}", *count);
//!         // Simulate async work
//!         tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
//!     }
//! }).await?;
//!
//! // Start the scheduler
//! cron.start().await;
//!
//! // Let it run for a few seconds
//! tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
//!
//! // Stop the scheduler
//! cron.stop().await;
//!
//! let final_count = *counter.lock().await;
//! println!("Final count: {}", final_count);
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Timezone Support Example
//!
//! ```rust
//! use chrono::{FixedOffset, TimeZone, Utc};
//! use cron_tab::Cron;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Tokyo timezone (UTC+9)
//! let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
//! let mut cron_tokyo = Cron::new(tokyo_tz);
//!
//! // New York timezone (UTC-5)
//! let ny_tz = FixedOffset::west_opt(5 * 3600).unwrap();
//! let mut cron_ny = Cron::new(ny_tz);
//!
//! // Jobs will run according to their respective timezones
//! cron_tokyo.add_fn("0 0 9 * * * *", || {
//!     println!("Good morning from Tokyo!");
//! })?;
//!
//! cron_ny.add_fn("0 0 9 * * * *", || {
//!     println!("Good morning from New York!");
//! })?;
//!
//! cron_tokyo.start();
//! cron_ny.start();
//! # Ok(())
//! # }
//! ```

mod error;

#[cfg(feature = "async")]
mod async_cron;
#[cfg(feature = "async")]
mod async_entry;

#[cfg(feature = "sync")]
mod cron;
#[cfg(feature = "sync")]
mod entry;

/// Re-export of the asynchronous cron scheduler.
///
/// This is only available when the "async" feature is enabled.
/// Use [`AsyncCron`] for asynchronous job scheduling with tokio runtime.
#[cfg(feature = "async")]
pub use crate::async_cron::AsyncCron;

/// Re-export of the synchronous cron scheduler.
///
/// This is only available when the "sync" feature is enabled.
/// Use [`Cron`] for synchronous job scheduling with threads.
#[cfg(feature = "sync")]
pub use crate::cron::Cron;

/// Re-export of the cron error type.
///
/// All fallible operations in this crate return a `Result<T, CronError>`.
pub use crate::error::CronError;

/// Convenience type alias for Results returned by this crate.
///
/// This is equivalent to `std::result::Result<T, CronError>`.
pub type Result<T> = std::result::Result<T, CronError>;

/// Maximum wait time in seconds for the scheduler loop.
///
/// This constant is used as a fallback when no jobs are scheduled,
/// preventing infinite blocking while still being responsive to
/// new job additions or stop signals.
const MAX_WAIT_SECONDS: u64 = 100000000;
