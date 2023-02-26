use std::{pin::Pin, sync::Arc};

use chrono::{DateTime, TimeZone};
use core::fmt;
use cron;
use futures::Future;

pub type TaskFuture = Box<dyn Future<Output = ()> + Send>;

pub trait TaskFuturePinned {
    fn get_pinned(&self) -> Pin<TaskFuture>;
}

pub struct TaskWrapper<F, T>(F)
where
    F: Fn() -> T,
    T: Future;

impl<F, T> TaskWrapper<F, T>
where
    F: Fn() -> T,
    T: Future,
{
    pub fn new(f: F) -> Self {
        TaskWrapper(f)
    }
}

impl<F, T> TaskFuturePinned for TaskWrapper<F, T>
where
    F: Fn() -> T,
    T: Future<Output = ()> + Send + 'static,
{
    fn get_pinned(&self) -> Pin<TaskFuture> {
        Box::pin(self.0())
    }
}

#[derive(Clone)]
pub struct AsyncEntry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    pub id: usize,
    pub schedule: cron::Schedule,
    pub next: Option<DateTime<Z>>,
    pub run: Arc<dyn TaskFuturePinned + Send + Sync>,
}

impl<Z> AsyncEntry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    pub fn get_next(&self, tz: Z) -> Option<DateTime<Z>> {
        self.schedule.upcoming(tz).next()
    }
}

impl<Z> fmt::Debug for AsyncEntry<Z>
where
    Z: Send + Sync + 'static,
    Z: TimeZone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {:?}", self.id, self.schedule, self.next)
    }
}
