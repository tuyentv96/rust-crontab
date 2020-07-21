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
//! cron_tab = "0.1.0"
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
//! extern crate cron_tab;
//!
//! use chrono::{FixedOffset, Local, TimeZone, Utc};
//!
//! fn main() {
//!     let local_tz = Local::from_offset(&FixedOffset::east(7));
//!     let utc_tz = Utc;
//!
//!     let mut cron = cron_tab::Cron::new(utc_tz);
//!
//!     let job_test_id = cron.add_fn("* * * * * * *", test).unwrap();
//!
//!     cron.start();
//!
//!     std::thread::sleep(std::time::Duration::from_secs(2));
//!     let anonymous_job_id = cron
//!         .add_fn("* * * * * *", || {
//!             println!("anonymous fn");
//!         })
//!         .unwrap();
//!
//!     // remove job_test
//!     cron.remove(job_test_id);
//!
//!     loop {
//!         std::thread::sleep(std::time::Duration::from_secs(2));
//!     }
//! }
//!
//! fn test() {
//!     println!("now: {}", Local::now().to_string());
//! }
//!
//! ```

#[macro_use]
extern crate crossbeam_channel;

use chrono::{DateTime, TimeZone, Utc};
use cron;
use std::ops::{DerefMut, Sub};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;
use thiserror;

#[derive(thiserror::Error, Debug)]
pub enum CronError {
    #[error("parse cron expression failed: {0}")]
    ParseError(#[from] cron::error::Error),
    #[error("unknown error")]
    Unknown,
}

#[derive(Clone)] // we implement the Copy trait
/// Entry wrap data about the job
pub struct Entry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    pub id: i32,
    pub schedule: cron::Schedule,
    pub next: Option<DateTime<Z>>,
    pub run: Arc<Mutex<dyn FnMut() + Send + Sync + 'static>>,
}

impl<Z> Entry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    fn get_next(&self, tz: Z) -> Option<DateTime<Z>> {
        self.schedule.upcoming(tz).next()
    }
}

#[derive(Clone)]
pub struct Cron<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
    <Z as TimeZone>::Offset: Send,
{
    entries: Arc<Mutex<Vec<Entry<Z>>>>,
    next_id: Arc<Mutex<i32>>,
    running: Arc<Mutex<bool>>,
    tz: Z,
    add_tx: crossbeam_channel::Sender<Entry<Z>>,
    add_rx: crossbeam_channel::Receiver<Entry<Z>>,
    remove_tx: crossbeam_channel::Sender<i32>,
    remove_rx: crossbeam_channel::Receiver<i32>,
    stop_tx: crossbeam_channel::Sender<bool>,
    stop_rx: crossbeam_channel::Receiver<bool>,
}

/// Cron contains and executes the scheduled jobs.
impl<Z> Cron<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
    <Z as TimeZone>::Offset: Send,
{
    /// Create a new cron.
    ///
    /// ```rust,ignore
    /// let mut cron = cron::Cron::new(Utc);
    /// cron.add_fn("* * * * * *", || {
    /// println!("anonymous fn");
    /// }).unwrap();
    /// cron.start();    
    /// ```
    pub fn new(tz: Z) -> Cron<Z> {
        let (add_tx, add_rx) = crossbeam_channel::unbounded();
        let (remove_tx, remove_rx) = crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = crossbeam_channel::unbounded();
        Cron {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(0)),
            running: Arc::new(Mutex::new(false)),
            // offset: None,
            tz: tz,
            add_tx,
            add_rx,
            remove_tx,
            remove_rx,
            stop_tx,
            stop_rx,
        }
    }

    /// Add a function to Cron.
    ///
    /// ```rust,ignore
    /// let mut cron = cron::Cron::new(Utc);
    /// cron.add_fn("* * * * * *", || {
    /// println!("anonymous fn");
    /// }).unwrap();
    /// cron.start();    
    /// ```
    pub fn add_fn<T>(&mut self, spec: &str, f: T) -> Result<i32, CronError>
    where
        T: 'static,
        T: FnMut() -> () + Send + Sync,
    {
        let schedule = cron::Schedule::from_str(spec)?;
        self.schedule(schedule, f)
    }

    fn schedule<T>(&mut self, schedule: cron::Schedule, f: T) -> Result<i32, CronError>
    where
        T: Send + Sync + 'static,
        T: FnMut() -> (),
    {
        let mut next_id = self.next_id.lock().unwrap();
        *next_id += 1;

        let entry = Entry {
            id: *next_id,
            schedule,
            next: None,
            run: Arc::new(Mutex::new(f)),
        };

        if *self.running.lock().unwrap() {
            self.add_tx.send(entry).unwrap();
        } else {
            self.entries.lock().unwrap().push(entry);
        }

        Ok(*next_id)
    }

    /// Set timezone offset.
    ///
    /// ```rust,ignore
    /// let mut cron = cron::Cron::new(Utc);
    /// cron.start();    
    /// ```
    pub fn set_timezone(&mut self, tz: Z) {
        self.tz = tz;
    }

    fn get_timezone(&self) -> Z {
        self.tz.clone()
    }

    fn now(&self) -> DateTime<Z> {
        self.get_timezone()
            .from_utc_datetime(&Utc::now().naive_utc())
    }

    fn remove_entry(&self, id: i32) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(index) = entries.iter().position(|e| e.id == id) {
            entries.remove(index);
        }
    }

    /// Remove a job from Cron.
    ///
    /// ```rust,ignore
    /// let mut cron = cron::Cron::new();
    /// let job_id = cron.add_fn("* * * * * *", || {
    /// println!("anonymous fn");
    /// }).unwrap();
    /// cron.start();  
    /// cron.remove(job_id);  
    /// ```
    pub fn remove(&self, id: i32) {
        if *self.running.lock().unwrap() {
            self.remove_tx.send(id).unwrap();
            return;
        }

        self.remove_entry(id);
    }

    /// Stop Cron.
    ///
    /// ```rust,ignore
    /// let mut cron = cron::Cron::new();
    /// let job_id = cron.add_fn("* * * * * *", || {
    /// println!("anonymous fn");
    /// }).unwrap();
    /// cron.start();  
    /// cron.stop();  
    /// ```
    pub fn stop(&self) {
        self.stop_tx.send(true).unwrap();
    }

    /// Start cron.
    /// A thead will be spawn for schedule jobs
    ///
    /// ```rust,ignore
    /// let mut cron = cron::Cron::new(Utc);
    /// let job_id = cron.add_fn("* * * * * *", || {
    /// println!("anonymous fn");
    /// }).unwrap();
    /// cron.start();
    /// ```
    pub fn start(&mut self) {
        let mut cloned_cron = self.clone();
        thread::spawn(move || {
            cloned_cron.run();
        });
    }

    /// Run a loop for schedule jobs
    fn run(&mut self) {
        let mut running = self.running.lock().unwrap();
        *running = true;
        drop(running);

        for entry in self.entries.lock().unwrap().iter_mut() {
            entry.next = entry.get_next(self.get_timezone());
        }

        // default long timer duration
        let mut timer_dur = time::Duration::from_secs(100000000);

        loop {
            self.entries
                .lock()
                .unwrap()
                .sort_by(|b, a| b.next.cmp(&a.next));

            if self.entries.lock().unwrap().len() > 0 {
                // get first entry from sorted entries for timer duration
                let dur = self.entries.lock().unwrap()[0]
                    .next
                    .as_ref()
                    .unwrap()
                    .timestamp_millis()
                    .sub(self.now().timestamp_millis());
                if dur > 0 {
                    if self.entries.lock().unwrap().len() > 0 {
                        timer_dur = time::Duration::from_millis(dur as u64);
                    }
                }
            }

            select! {
                // wait timer expire
                recv(crossbeam_channel::after(timer_dur)) -> _ => {
                    let now = Utc::now();
                    for entry in self.entries.lock().unwrap().iter_mut() {
                        if entry.next.as_ref().unwrap().gt(&now) {
                            break;
                        }

                        (entry.run.lock().unwrap().deref_mut())();
                        entry.next = entry.get_next(self.get_timezone());
                    }
                },
                // wait add new entry
                recv(self.add_rx) -> new_entry => {
                    let mut entry = new_entry.unwrap();
                    entry.next = entry.get_next(self.get_timezone());
                    self.entries.lock().unwrap().push(entry);
                },
                // wait remove entry
                recv(self.remove_rx) -> id => {
                    self.remove_entry(id.unwrap());
                },
                // wait stop cron
                recv(self.stop_rx) -> _ => {
                    return;
                },
            }
        }
    }
}
