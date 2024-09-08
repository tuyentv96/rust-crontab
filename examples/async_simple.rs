use std::sync::Arc;

use chrono::{FixedOffset, Local, TimeZone};
use cron_tab::AsyncCron;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
    let mut cron = AsyncCron::new(local_tz);

    cron.start().await;

    let counter = Arc::new(Mutex::new(1));
    cron.add_fn("* * * * * *", move || {
        let counter = counter.clone();
        async move {
            let mut counter = counter.lock().await;
            *counter += 1;
            let now = Local::now().to_string();
            println!("{} counter value: {}", now, counter);
        }
    })
    .await
    .unwrap();

    std::thread::sleep(std::time::Duration::from_secs(10));

    // stop cron
    cron.stop().await;
}
