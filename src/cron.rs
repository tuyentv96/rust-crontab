use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use crossbeam_channel;

use crate::entry::Entry;
use crate::Result;
use crate::MAX_WAIT_SECONDS;

#[derive(Clone)]
pub struct Cron<Z>
where
    Z: TimeZone + Sync + Send + 'static,
    Z::Offset: Send,
{
    entries: Arc<Mutex<Vec<Entry<Z>>>>,
    next_id: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    tz: Z,
    add_tx: Option<crossbeam_channel::Sender<Entry<Z>>>,
    remove_tx: Option<crossbeam_channel::Sender<usize>>,
    stop_tx: Option<crossbeam_channel::Sender<bool>>,
}

/// Cron contains and executes the scheduled jobs.
impl<Z> Cron<Z>
where
    Z: TimeZone + Sync + Send + 'static,
    Z::Offset: Send,
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
        Cron {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicUsize::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            tz,
            add_tx: None,
            remove_tx: None,
            stop_tx: None,
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
    pub fn add_fn<T>(&mut self, spec: &str, f: T) -> Result<usize>
    where
        T: 'static,
        T: Fn() -> () + Send + Sync,
    {
        let schedule = cron::Schedule::from_str(spec)?;
        self.schedule(schedule, f)
    }

    fn schedule<T>(&mut self, schedule: cron::Schedule, f: T) -> Result<usize>
    where
        T: Send + Sync + 'static,
        T: Fn() -> (),
    {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let mut entry = Entry {
            id: next_id,
            next: None,
            run: Arc::new(f),
            schedule,
        };

        entry.next = entry.schedule_next(self.get_timezone());

        match self.add_tx.as_ref() {
            Some(tx) if self.running.load(Ordering::SeqCst) => tx.send(entry).unwrap(),
            _ => self.entries.lock().unwrap().push(entry),
        }

        Ok(next_id)
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

    fn remove_entry(&self, id: usize) {
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
    pub fn remove(&self, id: usize) {
        if self.running.load(Ordering::SeqCst) {
            if let Some(tx) = self.remove_tx.as_ref() {
                tx.send(id).unwrap();
            }

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
        if let Some(tx) = self.stop_tx.as_ref() {
            tx.send(true).unwrap();
        }
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
            cloned_cron.start_blocking();
        });
    }

    /// Run a loop for schedule jobs
    pub fn start_blocking(&mut self) {
        let (add_tx, add_rx) = crossbeam_channel::unbounded();
        let (remove_tx, remove_rx) = crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = crossbeam_channel::unbounded();

        self.add_tx = Some(add_tx);
        self.remove_tx = Some(remove_tx);
        self.stop_tx = Some(stop_tx);

        self.running.store(true, Ordering::SeqCst);

        for entry in self.entries.lock().unwrap().iter_mut() {
            entry.next = entry.schedule_next(self.get_timezone());
        }

        // default long timer duration
        let mut wait_duration = Duration::from_secs(MAX_WAIT_SECONDS);

        loop {
            let mut entries = self.entries.lock().unwrap();
            entries.sort_by(|b, a| b.next.cmp(&a.next));

            if let Some(entry) = entries.first() {
                // get first entry from sorted entries for timer duration
                let wait_milis = (entry.next.as_ref().unwrap().timestamp_millis() as u64)
                    .saturating_sub(self.now().timestamp_millis() as u64);

                wait_duration = Duration::from_millis(wait_milis);
            }

            // release lock
            drop(entries);

            crossbeam_channel::select! {
                // wait timer expire
                recv(crossbeam_channel::after(wait_duration)) -> _ => {
                    let now = self.now();
                    for entry in self.entries.lock().unwrap().iter_mut() {
                        if entry.next.as_ref().unwrap().gt(&now) {
                            break;
                        }

                    let run = entry.run.clone();
                    thread::spawn(move || {
                        run();
                    });

                    entry.next = entry.schedule_next(self.get_timezone());
                    }
                },
                // wait add new entry
                recv(add_rx) -> new_entry => {
                    let mut entry = new_entry.unwrap();
                    entry.next = entry.schedule_next(self.get_timezone());
                    self.entries.lock().unwrap().push(entry);
                },
                // wait remove entry
                recv(remove_rx) -> id => {
                    self.remove_entry(id.unwrap());
                },
                // wait stop cron
                recv(stop_rx) -> _ => {
                    return;
                },
            }
        }
    }
}
