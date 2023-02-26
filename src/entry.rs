use std::sync::Arc;

use chrono::{DateTime, TimeZone};

// use crate::EntryTimeZone;

#[derive(Clone)]
/// Entry wrap data about the job
pub struct Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
{
    pub id: usize,
    pub next: Option<DateTime<Z>>,
    pub run: Arc<dyn Fn() + Send + Sync + 'static>,
    pub schedule: cron::Schedule,
}

impl<Z> Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
{
    pub fn schedule_next(&self, tz: Z) -> Option<DateTime<Z>> {
        self.schedule.upcoming(tz).next()
    }
}
