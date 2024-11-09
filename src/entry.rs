use std::{fmt, sync::Arc};

use chrono::{DateTime, TimeZone};

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

impl<Z> fmt::Debug for Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
    Z::Offset: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("next", &self.next)
            .field("run", &"")
            .field("schedule", &self.schedule)
            .finish()
    }
}

impl<Z> Entry<Z>
where
    Z: TimeZone + Sync + Send + 'static,
{
    pub fn schedule_next(&self, tz: Z) -> Option<DateTime<Z>> {
        self.schedule.upcoming(tz).next()
    }
}
