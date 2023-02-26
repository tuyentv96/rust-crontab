//! # CronTab
//!
//! A cron job library for Rust.
//!
//! ## Usage
//!
//! Add cron_tab crate to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! cron_tab = "0.1"
//! ```
//!
//! Creating a schedule for a job is done using the `FromStr` impl for the
//! `Schedule` type of the [cron](https://github.com/tuyentv96/cron_tab) library.
//!
//! The scheduling format is as follows:
//!
//! ```text
//! sec   min   hour   day of month   month   day of week   year
//! *     *     *      *              *       *             *
//! ```
//!
//! A simple example:
//!
//! ```rust,ignore
//! use chrono::{FixedOffset, Local, TimeZone};
//! use cron_tab;
//!
//! fn main() {
//!     let local_tz = Local::from_offset(&FixedOffset::east(7));
//!     let mut cron = cron_tab::Cron::new(local_tz);
//!
//!     let first_job_id = cron.add_fn("* * * * * * *", print_now).unwrap();
//!
//!     // start cron in background
//!     cron.start();
//!
//!     cron.add_fn("* * * * * *", move || {
//!         println!("add_fn {}", Local::now().to_string());
//!     })
//!     .unwrap();
//!
//!     // remove job_test
//!     cron.remove(first_job_id);
//!
//!     std::thread::sleep(std::time::Duration::from_secs(10));
//!
//!     // stop cron
//!     cron.stop();
//! }
//!
//! fn print_now() {
//!     println!("now: {}", Local::now().to_string());
//! }
//! ```
//!
//! Async example:
//! ```rust,ignore
//! use std::sync::Arc;
//!
//! use chrono::{FixedOffset, Local, TimeZone};
//! use cron_tab::AsyncCron;
//! use tokio::sync::Mutex;
//!
//! #[tokio::main]
//! async fn main() {
//!     let local_tz = Local::from_offset(&FixedOffset::east(7));
//!     let mut cron = AsyncCron::new(local_tz);
//!
//!     let first_job_id = cron.add_fn("* * * * * *", print_now).await;
//!
//!     cron.start().await;
//!
//!     let counter = Arc::new(Mutex::new(1));
//!     cron.add_fn("* * * * * *", move || {
//!         let counter = counter.clone();
//!         async move {
//!             let mut counter = counter.lock().await;
//!             *counter += 1;
//!             let now = Local::now().to_string();
//!             println!("{} counter value: {}", now, counter);
//!         }
//!     })
//!     .await;
//!
//!     std::thread::sleep(std::time::Duration::from_secs(10));
//!
//!     // stop cron
//!     cron.stop();
//! }
//!
//! async fn print_now() {
//!     println!("now: {}", Local::now().to_string());
//! }
//! ```

mod async_cron;
mod async_entry;
mod cron;
mod entry;
mod error;

pub use crate::{async_cron::AsyncCron, cron::Cron, error::CronError};

pub type Result<T> = std::result::Result<T, CronError>;

const MAX_WAIT_SECONDS: u64 = 100000000;
